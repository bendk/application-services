# Async bridges in Rust code

* Status: proposed
* Deciders: ?
* Date: October 5, 2025
* Feedback deadline: October 19, 2025.

## Context and Problem Statement

Many Rust APIs are both synchronous and blocking.
`rusqlite` in the prime example but there are other sources,
for example File IO and long-running computations.
This presents a problem since components are usually called from the application main thread,
where blocking is not allowed.

A typical solution for this is an async bridge function,
for example
[withContext(Dispatchers.IO)](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines/with-context.html) in Kotlin
or [spawn_blocking](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html) in tokio.
These functions schedule the blocking operation in a background thread
then wait for the result asynchronously.
Our current approach is to expose a sync/blocking Rust function via UniFFI,
then wrap that function in the application layer.
However, that approach has several issues.

The main issue with this approach is that it's not clear which functions
need to be wrapped with the async bridge.
Most functions should be wrapped, but some shouldn't.
Cancel functions shouldn't be wrapped since we want them to run immediately.
Constructors may or may not need wrapping:
LoginStore does since it eagerly opens the SQLite database,
but Suggest store doesn't since it opens it's database lazily.
This ambiguity adds extra work for the application teams
and requires cross-team collaboration to ensure the components are used correctly.

A second issue is that Desktop needs to be special cased.
JavaScript can't natively schedule work in a background thread,
so the async bridging happens in the C++ glue layer.
This is configured by a TOML file that specifies how each function is wrapped.
This is not an inherently terrible system,
but the fact that it only applies to Desktop can make it feel out-of-place.

Lastly, wrapping the entire function in an async bridge means that
we're always using a thread for the operation.
This includes times when the component is waiting on a mutex, waiting on user input, etc.
If we want to avoid using a thread during those times, we need to rework the system.

## Proposal: Move the async bridges to Rust

UniFFI has had the ability to generate async code for several years now
and we've been slowly introducing async code to application-services.
We can use this functionality to implement an async bridge in Rust
and move it out of the application layer.

### Define the WorkQueue interface

Define a UniFFI trait interface for running tasks in a work queue:

```rust
#[uniffi::export]
pub trait WorkQueue {
    fn run(&self, task: Arc<dyn Fn()>);
}
```

Notes:
  - This depends on UniFFI support for closures (`Arc<dyn Fn()>`) which doesn't exist yet.
    However, it seems possible to implement and it would be a nice UniFFI feature.
    If we can't do this, then we can use trait interface with a single method instead.
  - Task doesn't return any which simplifies the foreign implementations.
    We can still handle return values in our async bridge using `oneshot::Channel`

### Implement WorkQueue for all applications

The Kotlin and Swift implementations are fairly straightforward:

```kotlin
class KotlinWorkQueue : WorkQueue {
    fun run(task: () -> Unit) {
        withContext(Dispatchers.IO) {
            task()
        }
    }
}
```

```swift
class SwiftWorkQueue : WorkQueue {
    let queue = DispatchQueue.global(qos: .background)

    func run(task: () -> ()) {
        queue.async {
            task()
        }
    }
}
```

WorkQueue can't be implemented in JS.
However UniFFI traits can also be implemented in Rust,
which is how we would do it for Desktop:

```rust
pub struct DesktopWorkQueue;

impl WorkQueue for DesktopWorkQueue {
    fn run(&self, task: Arc<dyn Fn()>) {
        xpcom::moz_task::dispatch_background_task(
            "rust-components-desktop-work-queue",
            task,
        );
    }
}
```

### Implement spawn_blocking

```rust
pub async fn spawn_blocking<F, T>(task: F) -> T
    where F: FnOnce() -> T
{
    let (sender, receiver) = oneshot::channel();
    // On startup, we register a single global work queue for the application
    GLOBAL_WORK_QUEUE.run(Arc::new(move || {
        let task_result = task();
        sender.send(task_result);
    }));
    receiver.await
}
```
### Remove async bridges from the application layer

```kotlin
override suspend fun add(entry: LoginEntry) =
    // Remove the withContext() call in the line below
    withContext(coroutineContext) {
        getStorage().add(entry.toLoginEntry()).toLogin()
    }
```

Swift would have similar changes.

On Desktop, we would remove the async wrapper config.
In the short-term, we'd still need to configure sync functions to not have the async wrapping.
Eventually we can make that the default.

### Add async bridges to the Rust code

This just means wrapping all the existing code with `spawn_blocking`:

```rust
pub fn add(&self, entry: LoginEntry) -> ApiResult<Login> {
    spawn_blocking(move || {
        // code here, no additional changes needed
    })
}
```

### Future work: consider refactorings to block threads less often

At some point in the future, we could consider refactoring the Rust code to block threads less
often:

```rust
pub fn add(&self, entry: LoginEntry) -> ApiResult<Login> {
    // Unlock the DB and get the encryption key asynchronously
    let db = self.db_mutex.lock().await;
    let key = self.key_manager.get_key().await;
    spawn_blocking(move || {
        // Refactor this code to input `db` and `key`
    })
}
```

The benefit here is that we won't block a thread while waiting for the DB mutex
or getting the local encryption key.
The latter means we could update `get_key` to display the primary password dialog if needed
which would reduce some complexity in the JS code.

### Consider making `spawn_blocking` more ergonomic

Mark points out that calling spawn_blocking
and passing it a move closure might be off-putting for new Rust developers.
We could mitigate this by defining a `spawn_blocking` macro:

```rust
pub fn add(&self, entry: LoginEntry) -> ApiResult<Login> {
    spawn_blocking! {
        // Rust code here
    }.await
}
```

## Considered Options

* Keep current behavior
* Move async bridges to Rust

## Decision Outcome: Move async bridges to Rust

### Comparison to other options

Pros:
- Application teams don't need to consider async bridging.
  The new API contract is that all functions are safe to call from the main thread.
- JS doesn't need a special-cased system
- Opens the door to future refactors that reduce thread usage

Cons:
- Requires 1-2 extra FFI calls
- Adds some complexity to the Rust code, which could be intimidating to new Rust devs.
