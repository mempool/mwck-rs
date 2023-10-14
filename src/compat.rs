use std::time::Duration;
use tokio::task::JoinHandle;
use futures_util::Future;

pub fn spawn<F>(future: F) -> Option<JoinHandle<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    Some(tokio::task::spawn(future))
}

#[cfg(target_arch = "wasm32")]
pub fn spawn<F>(future: F) -> Option<JoinHandle<F::Output>>
where
    F: Future<Output = ()> + 'static
{
  wasm_bindgen_futures::spawn_local(future);
  None
}

#[must_use]
pub fn now() -> Duration {
  #[cfg(target_arch = "wasm32")]
  return instant::SystemTime::now()
      .duration_since(instant::SystemTime::UNIX_EPOCH)
      .unwrap_or_else(|_| unreachable!("System time before UNIX EPOCH"));

  #[cfg(not(target_arch = "wasm32"))]
  return std::time::SystemTime::now()
      .duration_since(std::time::SystemTime::UNIX_EPOCH)
      .unwrap_or_else(|_| unreachable!("System time before UNIX EPOCH"));
}


pub async fn sleep(delay: u64) {
  #[cfg(target_arch = "wasm32")]
  {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        let _ = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay as i32);};

    let p = js_sys::Promise::new(&mut cb);

    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
  }

  #[cfg(not(target_arch = "wasm32"))]
  return tokio::time::sleep(Duration::from_millis(delay)).await;
}