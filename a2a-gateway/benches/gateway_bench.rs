//! Performance benchmarks for the A2A Gateway

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::time::Duration;
use tokio::runtime::Runtime;
use url::Url;

use a2a_gateway::{
    domain::{ServiceInfo, ServiceRegistry, RequestContext, LoadBalancingStrategy},
    application::LoadBalancer,
    port::LoadBalancing,
};

/// Benchmark service registry operations
fn bench_service_registry(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("service_registry_register", |b| {
        b.to_async(&rt).iter(|| async {
            let registry = ServiceRegistry::new();
            let service = ServiceInfo::new(
                "bench-service".to_string(),
                Url::parse("http://localhost:3001").unwrap(),
            );
            
            black_box(registry.register(service).await.unwrap());
        });
    });
    
    c.bench_function("service_registry_get_all", |b| {
        b.to_async(&rt).iter_setup(
            || {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    let registry = ServiceRegistry::new();
                    
                    // Pre-populate with services
                    for i in 0..100 {
                        let service = ServiceInfo::new(
                            format!("service-{}", i),
                            Url::parse(&format!("http://localhost:{}", 3000 + i)).unwrap(),
                        );
                        registry.register(service).await.unwrap();
                    }
                    
                    registry
                })
            },
            |registry| async move {
                black_box(registry.get_all().await.unwrap());
            }
        );
    });
    
    c.bench_function("service_registry_get_healthy", |b| {
        b.to_async(&rt).iter_setup(
            || {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    let registry = ServiceRegistry::new();
                    
                    // Pre-populate with services
                    for i in 0..100 {
                        let service = ServiceInfo::new(
                            format!("service-{}", i),
                            Url::parse(&format!("http://localhost:{}", 3000 + i)).unwrap(),
                        );
                        let service_id = service.id.clone();
                        registry.register(service).await.unwrap();
                        
                        // Mark half as healthy
                        if i % 2 == 0 {
                            registry.update_status(&service_id, a2a_gateway::domain::ServiceStatus::Healthy).await.unwrap();
                        }
                    }
                    
                    registry
                })
            },
            |registry| async move {
                black_box(registry.get_healthy().await.unwrap());
            }
        );
    });
}

/// Benchmark load balancer operations
fn bench_load_balancer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("load_balancer_select");
    
    for service_count in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("round_robin", service_count),
            service_count,
            |b, &service_count| {
                b.to_async(&rt).iter_setup(
                    || {
                        let services: Vec<ServiceInfo> = (0..service_count)
                            .map(|i| {
                                let mut service = ServiceInfo::new(
                                    format!("service-{}", i),
                                    Url::parse(&format!("http://localhost:{}", 3000 + i)).unwrap(),
                                );
                                service.update_status(a2a_gateway::domain::ServiceStatus::Healthy);
                                service
                            })
                            .collect();
                        
                        let context = RequestContext::new("/test".to_string(), "GET".to_string());
                        let load_balancer = LoadBalancer::new();
                        
                        (services, context, load_balancer)
                    },
                    |(services, context, load_balancer)| async move {
                        black_box(
                            load_balancer
                                .select_service(&services, LoadBalancingStrategy::RoundRobin, &context)
                                .await
                                .unwrap()
                        );
                    }
                );
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("weighted_round_robin", service_count),
            service_count,
            |b, &service_count| {
                b.to_async(&rt).iter_setup(
                    || {
                        let services: Vec<ServiceInfo> = (0..service_count)
                            .map(|i| {
                                let mut service = ServiceInfo::new(
                                    format!("service-{}", i),
                                    Url::parse(&format!("http://localhost:{}", 3000 + i)).unwrap(),
                                ).with_weight((i % 5 + 1) as u32 * 10); // Varying weights
                                service.update_status(a2a_gateway::domain::ServiceStatus::Healthy);
                                service
                            })
                            .collect();
                        
                        let context = RequestContext::new("/test".to_string(), "GET".to_string());
                        let load_balancer = LoadBalancer::new();
                        
                        (services, context, load_balancer)
                    },
                    |(services, context, load_balancer)| async move {
                        black_box(
                            load_balancer
                                .select_service(&services, LoadBalancingStrategy::WeightedRoundRobin, &context)
                                .await
                                .unwrap()
                        );
                    }
                );
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("least_connections", service_count),
            service_count,
            |b, &service_count| {
                b.to_async(&rt).iter_setup(
                    || {
                        let rt = Runtime::new().unwrap();
                        rt.block_on(async {
                            let services: Vec<ServiceInfo> = (0..service_count)
                                .map(|i| {
                                    let mut service = ServiceInfo::new(
                                        format!("service-{}", i),
                                        Url::parse(&format!("http://localhost:{}", 3000 + i)).unwrap(),
                                    );
                                    service.update_status(a2a_gateway::domain::ServiceStatus::Healthy);
                                    service
                                })
                                .collect();
                            
                            let context = RequestContext::new("/test".to_string(), "GET".to_string());
                            let load_balancer = LoadBalancer::new();
                            
                            // Pre-populate with some connection counts
                            for (i, service) in services.iter().enumerate() {
                                load_balancer.update_connections(&service.id, (i % 10) as i32).await.unwrap();
                            }
                            
                            (services, context, load_balancer)
                        })
                    },
                    |(services, context, load_balancer)| async move {
                        black_box(
                            load_balancer
                                .select_service(&services, LoadBalancingStrategy::LeastConnections, &context)
                                .await
                                .unwrap()
                        );
                    }
                );
            },
        );
    }
    
    group.finish();
}

/// Benchmark request context operations
fn bench_request_context(c: &mut Criterion) {
    c.bench_function("request_context_creation", |b| {
        b.iter(|| {
            let context = RequestContext::new(
                black_box("/api/v1/tasks/send".to_string()),
                black_box("POST".to_string()),
            )
            .with_header("content-type".to_string(), "application/json".to_string())
            .with_header("authorization".to_string(), "Bearer token123".to_string())
            .with_param("taskId".to_string(), "task-456".to_string())
            .with_client_ip("192.168.1.100".to_string())
            .with_metadata("trace-id".to_string(), "trace-789".to_string());
            
            black_box(context);
        });
    });
}

/// Benchmark service filtering operations
fn bench_service_filtering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("service_filter_by_tags", |b| {
        b.to_async(&rt).iter_setup(
            || {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    let registry = ServiceRegistry::new();
                    
                    // Create services with different tags
                    let tags_sets = vec![
                        vec!["web".to_string(), "frontend".to_string()],
                        vec!["api".to_string(), "backend".to_string()],
                        vec!["database".to_string(), "storage".to_string()],
                        vec!["cache".to_string(), "redis".to_string()],
                        vec!["queue".to_string(), "messaging".to_string()],
                    ];
                    
                    for i in 0..100 {
                        let service = ServiceInfo::new(
                            format!("service-{}", i),
                            Url::parse(&format!("http://localhost:{}", 3000 + i)).unwrap(),
                        ).with_tags(tags_sets[i % tags_sets.len()].clone());
                        
                        registry.register(service).await.unwrap();
                    }
                    
                    registry
                })
            },
            |registry| async move {
                black_box(
                    registry.get_by_tags(&["api".to_string()]).await.unwrap()
                );
            }
        );
    });
}

criterion_group!(
    benches,
    bench_service_registry,
    bench_load_balancer,
    bench_request_context,
    bench_service_filtering
);
criterion_main!(benches);
