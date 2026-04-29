use cognitive::services::ppr_retrieval::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use petgraph::graph::DiGraph;

fn bench_ppr_small(c: &mut Criterion) {
    let g = build_chain_graph(50);
    let seeds = vec![petgraph::graph::NodeIndex::new(0)];
    c.bench_function("ppr_50_nodes", |b| {
        b.iter(|| {
            personalized_pagerank(black_box(&g), black_box(&seeds), &PprConfig::default());
        });
    });
}

fn bench_ppr_large(c: &mut Criterion) {
    let g = build_chain_graph(2000);
    let seeds = vec![petgraph::graph::NodeIndex::new(0)];
    c.bench_function("ppr_2000_nodes", |b| {
        b.iter(|| {
            personalized_pagerank(black_box(&g), black_box(&seeds), &PprConfig::default());
        });
    });
}

fn build_chain_graph(n: usize) -> DiGraph<String, f32, u32> {
    let mut g = DiGraph::new();
    let mut prev = None;
    for i in 0..n {
        let cur = g.add_node(format!("n{i}"));
        if let Some(p) = prev {
            g.add_edge(p, cur, 1.0);
            g.add_edge(cur, p, 1.0);
        }
        prev = Some(cur);
    }
    g
}

criterion_group!(benches, bench_ppr_small, bench_ppr_large);
criterion_main!(benches);
