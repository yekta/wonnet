//! A message's body: the core's page, as tall as the page, its links opening in the browser.
//! On Linux WPE WebKit paints it off the screen and the app draws it; elsewhere it's the
//! system's web view, laid over the window.

#[cfg(target_os = "linux")]
mod drawn;
#[cfg(not(target_os = "linux"))]
mod native;

#[cfg(target_os = "linux")]
pub use drawn::BodyView;
#[cfg(not(target_os = "linux"))]
pub use native::{BodyView, Occlusion};

const SCRIPT: &str = r#"
(function () {
  function fit(body) {
    body.style.width = ''; body.style.transform = '';
    var wide = document.documentElement.scrollWidth, shown = document.documentElement.clientWidth;
    if (wide <= shown + 1) { return; }
    body.style.width = wide + 'px'; body.style.transformOrigin = '0 0';
    body.style.transform = 'scale(' + shown / wide + ')';
  }
  function report() {
    var body = document.body;
    if (!body) { return; }
    fit(body);
    window.ipc.postMessage('height:' + Math.ceil(body.getBoundingClientRect().height));
  }
  function drawn() { window.ipc.postMessage('drawn'); }
  window.wonnetDrawn = drawn;
  document.addEventListener('DOMContentLoaded', function () {
    document.documentElement.style.overflow = 'hidden';
    new ResizeObserver(report).observe(document.body);
    report();
    drawn();
  });
  document.addEventListener('toggle', function () { report(); drawn(); }, true);
  document.addEventListener('load', drawn, true);
  window.addEventListener('load', function () { report(); drawn(); });
  window.addEventListener('resize', function () {
    report();
    window.scrollTo(0, window.cut || 0);
  });
  window.addEventListener('wheel', function (event) {
    event.preventDefault();
    window.ipc.postMessage('wheel:' + event.deltaY);
  }, { passive: false });
  document.addEventListener('keydown', function (event) {
    var copy = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'c';
    if (copy && String(window.getSelection())) {
      event.preventDefault();
      document.execCommand('copy');
      return;
    }
    event.preventDefault();
    window.ipc.postMessage('key:' + JSON.stringify({
      key: event.key, ctrl: event.ctrlKey, alt: event.altKey, shift: event.shiftKey, meta: event.metaKey
    }));
  }, true);
  document.addEventListener('mouseup', function () {
    setTimeout(function () {
      if (!String(window.getSelection())) { window.ipc.postMessage('done'); }
    }, 0);
  });
})();
"#;

/// The page in the app's scheme, whatever the system's: the core's CSS asks for
/// `prefers-color-scheme`, which is answered here.
fn themed(html: &str, dark: bool) -> String {
    let answer = if dark { "all" } else { "not all" };
    html.replace("@media (prefers-color-scheme: dark)", &format!("@media {answer}"))
}
