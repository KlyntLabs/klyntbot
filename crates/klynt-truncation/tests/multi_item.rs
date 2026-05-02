use klynt_truncation::{truncate_function_output_items, ContentItem, TruncationPolicy};

#[test]
fn images_are_preserved_text_is_truncated() {
    let items = vec![
        ContentItem::Text("a".repeat(10_000)),
        ContentItem::Image {
            url: "data:image/png;base64,AAA".into(),
        },
        ContentItem::Text("b".repeat(10_000)),
    ];

    let out = truncate_function_output_items(&items, TruncationPolicy::Bytes(500));

    let images: Vec<_> = out
        .iter()
        .filter(|i| matches!(i, ContentItem::Image { .. }))
        .collect();
    assert_eq!(images.len(), 1, "image must survive");

    let total_text_bytes: usize = out
        .iter()
        .filter_map(|i| match i {
            ContentItem::Text(t) => Some(t.len()),
            _ => None,
        })
        .sum();
    assert!(total_text_bytes <= 700, "text truncated: {total_text_bytes}");
}

#[test]
fn omitted_items_get_sentinel() {
    let items = vec![
        ContentItem::Text("a".repeat(1000)),
        ContentItem::Text("b".repeat(1000)),
        ContentItem::Text("c".repeat(1000)),
    ];
    let out = truncate_function_output_items(&items, TruncationPolicy::Bytes(900));
    let last = out.last().unwrap();
    if let ContentItem::Text(t) = last {
        assert!(t.contains("omitted"), "expected sentinel: {t}");
    } else {
        panic!("last item should be sentinel text");
    }
}
