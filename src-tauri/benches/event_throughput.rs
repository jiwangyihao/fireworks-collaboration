#![cfg(feature = "bench")] // 需要启用 bench feature 才编译
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fireworks_collaboration_lib::events::structured::{Event, MemoryEventBus, TaskEvent};
use std::sync::Arc;

fn bench_event_publish(c: &mut Criterion) {
    let bus = Arc::new(MemoryEventBus::new());
    c.bench_function("publish_1k_events", |b| {
        b.iter(|| {
            for i in 0..1000 {
                bus.publish(Event::Task(TaskEvent::Started {
                    id: format!("{}", i),
                    kind: "Bench".into(),
                }));
            }
            black_box(())
        })
    });
}

criterion_group!(benches, bench_event_publish);
criterion_main!(benches);
