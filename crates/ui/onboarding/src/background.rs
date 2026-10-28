use gpui_kit::{Context, Window};
use margin_workbench::{AppServices, run_blocking};

pub(crate) fn in_background<View: 'static, Output: Send + 'static>(
    window: &Window,
    cx: &Context<'_, View>,
    work: impl FnOnce(&AppServices) -> Output + Send + 'static,
    apply: impl FnOnce(&mut View, Output, &mut Window, &mut Context<'_, View>) + 'static,
) {
    let services = cx.global::<AppServices>().clone();
    let task = run_blocking(cx, move || work(&services));
    cx.spawn_in(window, async move |this, cx| {
        let output = task.await;
        this.update_in(cx, |view, window, cx| apply(view, output, window, cx))
            .unwrap_or_default();
    })
    .detach();
}
