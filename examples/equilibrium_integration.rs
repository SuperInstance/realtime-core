// Example: Integration with equilibrium-tokens Rate Equilibrium Surface
//
// This demonstrates how realtime-core provides the timing foundation
// for equilibrium-tokens to achieve <2ms jitter in rate control.

use equilibrium_tokens::RateEquilibrium;
use realtime_core::{
    RealtimeExecutor, ExecutorConfig, Scheduler, SchedulingPolicy, Timer,
};
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Real-time rate controller with <2ms jitter guarantee
///
/// This is the core component that equilibrium-tokens uses for
/// precise token emission timing.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Equilibrium Tokens + realtime-core ===\n");

    // Create rate equilibrium surface
    let rate_surface = RateEquilibrium::new(
        2.0,  // Target: 2 tokens/second
        0.5,  // Minimum: 0.5 Hz
        5.0,  // Maximum: 5 Hz
    )?;

    println!("Rate Equilibrium Surface created:");
    println!("  Target rate: {} Hz", rate_surface.current_rate());
    println!("  Interval: {} ms", 1000.0 / rate_surface.current_rate());
    println!();

    // Configure real-time executor with CPU isolation
    let executor = RealtimeExecutor::with_config(ExecutorConfig {
        cpu_affinity: Some(vec![1]),  // Isolate to CPU 1
        scheduling_policy: SchedulingPolicy::Deadline {
            runtime_ns: 1_000_000,     // 1ms CPU time per period
            deadline_ns: 500_000,      // Complete within 500µs
            period_ns: 500_000_000,    // 500ms period (2 Hz)
        },
        worker_threads: Some(1),
        enable_parking: false,  // Disable for low latency
        ..Default::default()
    })?;

    println!("Real-time executor configured:");
    println!("  CPU affinity: core 1");
    println!("  Scheduling: SCHED_DEADLINE");
    println!("  Runtime: 1ms, Deadline: 500µs, Period: 500ms");
    println!();

    // Spawn real-time rate control task
    let mut timer = Timer::new(2.0)?;
    let mut tick_count = 0u64;
    let mut jitter_stats = Vec::new();
    let mut last_tick = Instant::now();

    println!("Starting rate control loop (Ctrl+C to stop)...\n");

    loop {
        // Wait for precise tick with <2ms jitter
        timer.wait_for_tick().await?;

        let now = Instant::now();
        let actual_interval_ns = now.duration_since(last_tick).as_nanos() as u64;
        let expected_interval_ns = timer.interval_ns();
        let jitter_ns = (actual_interval_ns as i64 - expected_interval_ns as i64).abs();

        // Record jitter
        jitter_stats.push(jitter_ns);

        // Emit token through equilibrium surface
        match rate_surface.emit_token() {
            Ok(token) => {
                tick_count += 1;

                // Print stats every 10 ticks
                if tick_count % 10 == 0 {
                    let avg_jitter_ns =
                        jitter_stats.iter().sum::<u64>() / jitter_stats.len() as u64;
                    let max_jitter_ns = *jitter_stats.iter().max().unwrap_or(&0);

                    println!(
                        "Tick #{:04} | Jitter: avg={}µs, max={}µs | Rate: {:.2} Hz",
                        tick_count,
                        avg_jitter_ns / 1000,
                        max_jitter_ns / 1000,
                        rate_surface.current_rate()
                    );

                    jitter_stats.clear();
                }

                // Print jitter warnings
                if jitter_ns > 1_000_000 {
                    // Jitter > 1ms
                    eprintln!(
                        "  ⚠️  High jitter: {}µs (expected: {}µs)",
                        jitter_ns / 1000,
                        expected_interval_ns / 1000
                    );
                }

                if jitter_ns > 2_000_000 {
                    // Jitter > 2ms (exceeds target!)
                    eprintln!(
                        "  ❌ CRITICAL: Jitter {}µs exceeds 2ms target!",
                        jitter_ns / 1000
                    );
                }
            }
            Err(e) => {
                eprintln!("Error emitting token: {}", e);
            }
        }

        last_tick = now;

        // Optional: Rate adjustment based on external factors
        // This is where equilibrium-tokens adjusts rate based on:
        // - User sentiment (higher sentiment → faster rate)
        // - Conversation context (interruption → pause/slow)
        // - System load (overload → slow down)
        if tick_count % 50 == 0 {
            // Example: Adjust rate based on simulated sentiment
            let sentiment_adjustment = simulate_sentiment_adjustment();
            let new_rate = 2.0 * sentiment_adjustment;
            rate_surface.set_target_rate(new_rate)?;

            if (new_rate - 2.0).abs() > 0.1 {
                println!("  📊 Rate adjusted: {:.2} Hz (sentiment factor: {:.2})",
                    new_rate, sentiment_adjustment);

                // Recreate timer with new rate
                timer = Timer::new(new_rate)?;
            }
        }
    }
}

/// Simulate sentiment-based rate adjustment
///
/// In equilibrium-tokens, this would be:
/// - Higher sentiment (valence, arousal) → faster token emission
/// - Lower sentiment → slower token emission
/// - Interruption detected → pause emission
fn simulate_sentiment_adjustment() -> f64 {
    use std::time::SystemTime;

    // Simulate varying sentiment based on time
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Oscillate between 0.5x and 1.5x rate based on "sentiment"
    let sentiment_factor = (secs as f64 / 10.0).sin();
    1.0 + sentiment_factor * 0.5
}

// Example: Multi-rate coordination
//
// This shows how equilibrium-tokens coordinates multiple timing domains:
// - Rate control (2 Hz)
// - VAD processing (30 Hz)
// - Sentiment updates (10 Hz)
#[allow(dead_code)]
async fn multi_rate_coordination_example() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::select;

    println!("=== Multi-Rate Coordination ===\n");

    // Rate control: 2 Hz
    let mut rate_timer = Timer::new(2.0)?;

    // VAD: 30 Hz (30ms audio frames)
    let mut vad_timer = Timer::new(30.0)?;

    // Sentiment: 10 Hz (100ms updates)
    let mut sentiment_timer = Timer::new(10.0)?;

    let mut rate_count = 0u64;
    let mut vad_count = 0u64;
    let mut sentiment_count = 0u64;

    loop {
        tokio::select! {
            _ = rate_timer.wait_for_tick() => {
                rate_count += 1;
                println!("📝 Rate tick #{} (token emission)", rate_count);
            }
            _ = vad_timer.wait_for_tick() => {
                vad_count += 1;
                // Process VAD (Voice Activity Detection)
                if vad_count % 10 == 0 {
                    println!("🎤 VAD tick #{} (speech detected)", vad_count);
                }
            }
            _ = sentiment_timer.wait_for_tick() => {
                sentiment_count += 1;
                println!("💭 Sentiment tick #{} (VAD update)", sentiment_count);
            }
        }
    }
}

// Example: Error handling and recovery
#[allow(dead_code)]
async fn error_handling_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Error Handling and Recovery ===\n");

    let mut timer = Timer::new(2.0)?;
    let rate_surface = RateEquilibrium::new(2.0, 0.5, 5.0)?;

    loop {
        // Wait for tick with timeout
        match timeout(Duration::from_millis(100), timer.wait_for_tick()).await {
            Ok(Ok(())) => {
                // Tick received on time
                match rate_surface.emit_token() {
                    Ok(token) => {
                        println!("✅ Token emitted: {:?}", token);
                    }
                    Err(e) => {
                        eprintln!("❌ Token emission failed: {}", e);
                        // Recovery: continue, surface will attempt recovery
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("❌ Timer error: {}", e);
                // Recovery: recreate timer
                timer = Timer::new(2.0)?;
            }
            Err(_) => {
                eprintln!("⚠️  Timeout: tick delayed beyond 100ms");
                // Recovery: adjust for missed tick
                continue;
            }
        }
    }
}
