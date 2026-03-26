use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Manages async tasks and coordinates shutdown.
/// Gère les tâches asynchrones et coordonne l'arrêt.
pub struct Supervisor {
    cancel: CancellationToken,
    tasks: JoinSet<Result<(), String>>,
}

impl Supervisor {
    /// Create a new supervisor with the given cancellation token.
    /// Crée un nouveau superviseur avec le jeton d'annulation donné.
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            tasks: JoinSet::new(),
        }
    }

    /// Spawn a named async task.
    /// Lance une tâche asynchrone nommée.
    pub fn spawn<F>(&mut self, name: &'static str, future: F)
    where
        F: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        info!("Supervisor: spawning task '{}'", name);
        self.tasks.spawn(async move {
            let result = future.await;
            if let Err(ref e) = result {
                error!("Task '{}' failed: {}", name, e);
            } else {
                info!("Task '{}' completed", name);
            }
            result
        });
    }

    /// Wait for cancellation, then join all tasks.
    /// Attend l'annulation, puis rejoint toutes les tâches.
    pub async fn run(mut self) {
        // Wait until cancellation is triggered (by signal handler or fatal error)
        self.cancel.cancelled().await;
        info!("Supervisor: shutdown initiated, waiting for tasks...");

        // Give tasks a moment to finish gracefully
        while let Some(result) = self.tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => error!("Task error during shutdown: {}", e),
                Err(e) => error!("Task panicked during shutdown: {}", e),
            }
        }

        info!("Supervisor: all tasks completed");
    }

    /// Return a clone of the cancellation token.
    /// Retourne un clone du jeton d'annulation.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }
}
