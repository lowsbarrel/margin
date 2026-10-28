use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{Context, Task};
use margin_search::SearchMode;
use margin_workbench::{CurrentVault, run_blocking};

use crate::results::Results;
use crate::spotlight::Spotlight;

const DEBOUNCE: Duration = Duration::from_millis(90);
const MAX_NAME_RESULTS: usize = 8;
const MAX_CONTENT_RESULTS: u32 = 100;

type ResultsOf = fn(&mut Spotlight) -> &mut Results;

pub(crate) fn debounced_search(text: String, cx: &Context<'_, Spotlight>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(DEBOUNCE).await;
        drop(this.update(cx, |this, cx| this.run_search(text, cx)));
    })
}

pub(crate) fn search(text: String, results: ResultsOf, cx: &Context<'_, Spotlight>) -> Task<()> {
    let Some(services) = CurrentVault::get(cx) else {
        return Task::ready(());
    };
    let query = text.clone();
    let work = run_blocking(cx, move || {
        let mut names = services.files.search_files(&query);
        names.truncate(MAX_NAME_RESULTS);
        let hits = services
            .search
            .search(&query, SearchMode::All, MAX_CONTENT_RESULTS)
            .unwrap_or_default();
        (names, hits)
    });
    cx.spawn(async move |this, cx| {
        let (names, hits) = work.await;
        drop(this.update(cx, |this, cx| {
            let results = results(this);
            if results.query().search_text() == Some(text.as_str()) {
                results.set_search_results(names, hits);
                cx.notify();
            }
        }));
    })
}

pub(crate) fn tags(results: ResultsOf, cx: &Context<'_, Spotlight>) -> Task<()> {
    let Some(services) = CurrentVault::get(cx) else {
        return Task::ready(());
    };
    let search = Arc::clone(&services.search);
    let work = run_blocking(cx, move || search.tags().unwrap_or_default());
    cx.spawn(async move |this, cx| {
        let tags = work.await;
        drop(this.update(cx, |this, cx| {
            results(this).set_tags(tags);
            cx.notify();
        }));
    })
}
