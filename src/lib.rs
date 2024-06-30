use std::pin::Pin;
use std::task::{Context, Poll};
use futures::{Stream, Future};
use pin_project::pin_project;

#[pin_project]
pub struct AndFinally<Item, Fut, F, O> {
    #[pin]
    item: Option<Item>,
    #[pin]
    fut: Option<Fut>,
    f: F,
    output: Option<O>
}

pub trait AndFinallyExt: Sized {
    fn and_finally<Fut: Future, F: Fn() -> Fut, O>(self, f: F) -> AndFinally<Self, Fut, F, O> {
        AndFinally { item: Some(self), fut: None, f, output: None,}
    }
}

impl<T: Sized> AndFinallyExt for T {}

impl<Item: Stream, Fut: Future, F, O> Stream for AndFinally<Item, Fut, F, O> where F: Fn() -> Fut {
   type Item = Item::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some(item) = this.item.as_mut().as_pin_mut() {
            match futures::ready!(item.poll_next(cx)) {
                Some(item) => return Poll::Ready(Some(item)),
                None => {
                    let fut = Some((this.f)());
                    this.fut.set(fut);
                }
            };
        }

        this.item.as_mut().as_pin_mut().take();

        if let Some(fut) = this.fut.as_mut().as_pin_mut() {
            futures::ready!(fut.poll(cx));
        }

        this.fut.as_mut().as_pin_mut().take();

        Poll::Ready(None)
    }
}

impl<Item: Future<Output = O>, Fut: Future, F, O> Future for AndFinally<Item, Fut, F, O> where F: Fn() -> Fut, {
    type Output = Item::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let Some(item) = this.item.as_mut().as_pin_mut() {
            let output = futures::ready!(item.poll(cx));
            this.output.replace(output);
            let fut = Some((this.f)());
            this.fut.set(fut);
        }

        this.item.as_mut().as_pin_mut().take();

        if let Some(fut) = this.fut.as_mut().as_pin_mut() {
            futures::ready!(fut.poll(cx));
        }

        this.fut.as_mut().as_pin_mut().take();

        let output = this.output.take();

        Poll::Ready(output.expect("output from future to be value"))
    }
}