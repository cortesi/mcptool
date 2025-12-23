use std::{fmt::Display, future::Future, time::Instant};

use async_trait::async_trait;

use crate::output::Output;

#[async_trait]
pub trait TimedFuture: Future + Sized {
    async fn timed<S: AsRef<str> + Display + Send>(
        self,
        title: S,
        output: &Output,
    ) -> Self::Output {
        let start = Instant::now();
        let result = self.await;

        drop(output.text(format!(
            "{} in {:.2}ms",
            title,
            start.elapsed().as_secs_f64() * 1000.0,
        )));

        result
    }
}

impl<F: Future> TimedFuture for F {}
