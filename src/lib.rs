pub mod future {
    use pin_project::pin_project;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[pin_project]
    pub struct ThenFinally<FT, Fut, F, O> {
        #[pin]
        item: Option<FT>,
        #[pin]
        fut: Option<Fut>,
        f: Option<F>,
        output: Option<O>,
    }

    pub trait ThenFinallyFutureExt: Sized {
        /// Consumes the current future into a new one which will execute an asynchronous upon completion of the future
        ///
        /// Note that this will execute the code regardless of a value that the future returns.
        fn then_finally<Fut: Future, F: FnOnce() -> Fut, O>(
            self,
            f: F,
        ) -> ThenFinally<Self, Fut, F, O> {
            ThenFinally {
                item: Some(self),
                fut: None,
                f: Some(f),
                output: None,
            }
        }
    }

    impl<T: Sized> ThenFinallyFutureExt for T {}

    impl<FT: Future<Output = O>, Fut: Future, F, O> Future for ThenFinally<FT, Fut, F, O>
    where
        F: FnOnce() -> Fut,
    {
        type Output = FT::Output;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();

            if let Some(item) = this.item.as_mut().as_pin_mut() {
                let output = futures::ready!(item.poll(cx));
                this.output.replace(output);
                let func = this.f.take().expect("function is valid");
                let fut = Some(func());
                this.fut.set(fut);
                this.item.set(None);
            }

            if let Some(fut) = this.fut.as_mut().as_pin_mut() {
                futures::ready!(fut.poll(cx));
                this.fut.set(None);
            }

            let output = this.output.take();

            Poll::Ready(output.expect("output from future to be value"))
        }
    }
}

pub mod stream {
    use futures::{Future, Stream};
    use pin_project::pin_project;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[pin_project]
    pub struct Finally<ST, Fut, F> {
        #[pin]
        item: Option<ST>,
        #[pin]
        fut: Option<Fut>,
        f: Option<F>,
    }

    pub trait FinallyStreamExt: Sized {
        /// Consumes the current stream into a new one which will execute an asynchronous upon completion of the stream
        ///
        /// Note that this will execute the code regardless of a value that the stream returns.
        fn finally<Fut: Future, F: FnOnce() -> Fut>(self, f: F) -> Finally<Self, Fut, F> {
            Finally {
                item: Some(self),
                fut: None,
                f: Some(f),
            }
        }
    }

    impl<T: Sized> FinallyStreamExt for T {}

    impl<ST: Stream, Fut: Future, F> Stream for Finally<ST, Fut, F>
    where
        F: FnOnce() -> Fut,
    {
        type Item = ST::Item;
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();

            if let Some(item) = this.item.as_mut().as_pin_mut() {
                match futures::ready!(item.poll_next(cx)) {
                    Some(item) => return Poll::Ready(Some(item)),
                    None => {
                        let func = this.f.take().expect("function is valid");
                        let fut = Some(func());
                        this.fut.set(fut);
                        this.item.set(None);
                    }
                };
            }

            if let Some(fut) = this.fut.as_mut().as_pin_mut() {
                futures::ready!(fut.poll(cx));
                this.fut.set(None);
            }

            Poll::Ready(None)
        }
    }
}

pub mod try_stream {
    use futures::{Future, Stream, TryStream};
    use pin_project::pin_project;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[pin_project]
    pub struct TryFinally<ST, Fut, F>
    where
        ST: TryStream,
    {
        #[pin]
        item: Option<ST>,
        #[pin]
        fut: Option<Fut>,
        f: Option<F>,
    }

    pub trait FinallyTryStreamExt: Sized {
        fn try_finally<Fut: Future, F: FnOnce() -> Fut>(self, f: F) -> TryFinally<Self, Fut, F>
        where
            Self: TryStream,
        {
            TryFinally {
                item: Some(self),
                fut: None,
                f: Some(f),
            }
        }
    }

    impl<T: Sized> FinallyTryStreamExt for T {}

    impl<ST: TryStream, Fut: Future, F> Stream for TryFinally<ST, Fut, F>
    where
        F: FnOnce() -> Fut,
    {
        type Item = Result<ST::Ok, ST::Error>;
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();

            if let Some(item) = this.item.as_mut().as_pin_mut() {
                let result = futures::ready!(item.try_poll_next(cx));

                match result {
                    Some(Ok(val)) => return Poll::Ready(Some(Ok(val))),
                    Some(Err(e)) => {
                        this.item.set(None);
                        this.f.take();
                        return Poll::Ready(Some(Err(e)));
                    }
                    None => {
                        let func = this.f.take().expect("function is valid");
                        let fut = Some(func());
                        this.fut.set(fut);
                        this.item.set(None);
                    }
                }
            }

            if let Some(fut) = this.fut.as_mut().as_pin_mut() {
                futures::ready!(fut.poll(cx));
                this.fut.set(None);
            }

            Poll::Ready(None)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::future::ThenFinallyFutureExt;
    use crate::stream::FinallyStreamExt;
    use futures::{StreamExt, TryStreamExt};
    use std::convert::Infallible;
    use crate::try_stream::FinallyTryStreamExt;

    #[test]
    fn future_final() {
        futures::executor::block_on(async move {
            let mut val = 0;

            futures::future::ready(())
                .then_finally(|| async {
                    val = 1;
                })
                .await;

            assert_eq!(val, 1);
        });
    }

    #[test]
    fn stream_final() {
        futures::executor::block_on(async move {
            let mut val = 0;

            let st = futures::stream::once(async { 0 }).finally(|| async {
                val = 1;
            });

            futures::pin_mut!(st);

            while let Some(v) = st.next().await {
                assert_eq!(v, 0);
            }

            assert_eq!(val, 1);
        });
    }

    #[test]
    fn try_stream_final() {
        futures::executor::block_on(async move {
            let mut val = 0;

            let st = futures::stream::once(async { Ok::<_, Infallible>(0) }).try_finally(|| async {
                val = 1;
            });

            futures::pin_mut!(st);

            while let Ok(Some(v)) = st.try_next().await {
                assert_eq!(v, 0);
            }

            let st = futures::stream::once(async {
                Err::<i8, std::io::Error>(std::io::ErrorKind::Other.into())
            })
            .try_finally(|| async {
                unreachable!()
            });

            futures::pin_mut!(st);

            while let Ok(_) = st.try_next().await {
                unreachable!()
            }

            assert_eq!(val, 1);
        });
    }
}
