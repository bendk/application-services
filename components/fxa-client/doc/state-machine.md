# The Public FxA State Machine

The public FxA state machine tracks a user's authentication state as they perform operations on their account.
The state machine, its states, and its events are visible to the consumer applications.
Applications generally track the state and update the UI based on it, for example providing a login button for the `NotAuthenticated` state and link to the FxA account management page for the `Authenticated` state.

The public state machine events correspond to user actions, for example clicking the login button or completing the OAuth flow.
The public state machine is non-deterministic -- from a given state and event, there are multiple possibilities for the next state.
Usually there are two possible transitions: one for a successful operation and one for a failed one.
For example, when completing an oauth flow, if the operation is successful the state machine transitions to the `Authenticated` state, while if it fails it stays in the `Authenticating` state.

Here is an overview containing some of the states and transitions:

```mermaid
graph LR;
    NotAuthenticated --> |"BeginOAuthFlow(Success)"| Authenticating
    NotAuthenticated --> |"BeginOAuthFlow(Failure)"| NotAuthenticated
    NotAuthenticated --> |"BeginPairingFlow(Success)"| Authenticating
    NotAuthenticated --> |"BeginPairingFlow(Failure)"| NotAuthenticated
    Authenticating --> |"CompleteOAuthFlow(Success)"| Athenticated
    Authenticating --> |"CompleteOAuthFlow(Failure)"| Authenticating
    Authenticating --> |"CancelOAuthFlow"| NotAuthenticated
    Athenticated --> |"Disconnect"| NotAuthenticated

    classDef default fill:#0af, color:black, stroke:black
```

# The Internal State Machines

For public state, we also define an internal state machine that represents the process of transitioning out of that state.
Each state of the internal state machine represents a `FirefoxAccount` method call.
Each event represents the result of a call -- either a successful return or failure.
Unlike the public state machine, the internal state machines are deterministic.

The initial state of an internal state machine is determined by the event sent to the public state machine.
Each internal state machine has 2 kinds of terminal states:
  - `Complete`: Complete the process and transition the public state machine to a new state
  - `Cancel`: Complete the process, but don't change the public state machine's state.

Here are some example internal state machines:

## NotAuthenticated

```mermaid
graph TD;
    Athenticating["Complete(Athenticating)"]:::terminal
    BeginOAuthFlow --> |BeginOAuthFlowSuccess| Athenticating
    BeginPairingFlow --> |BeginPairingFlowSuccess| Athenticating
    BeginOAuthFlow --> |Error| Cancel:::terminal
    BeginPairingFlow --> |Error| Cancel:::terminal

    classDef default fill:#0af, color:black, stroke:black
    classDef terminal fill:#FC766A, stroke: black;
```

## Authenticating

```mermaid
graph TD;
    Authenticated["Complete(Authenticated)"]:::terminal
    CompleteOAuthFlow --> |CompleteOAuthFlowSuccess| InitializeDevice
    CompleteOAuthFlow --> |Error| Cancel:::terminal
    InitializeDevice --> |InitializeDeviceSuccess| Authenticated
    InitializeDevice --> |Error| Cancel:::terminal

    classDef default fill:#0af, color:black, stroke:black
    classDef terminal fill:#FC766A, stroke: black;
```

## Uninitialized

This state is not show in the diagram above, but it's the initial state for the public state machine.

```mermaid
graph TD;
    NotAuthenticated["Complete(NotAuthenticated)"]:::terminal
    Authenticated["Complete(Authenticated)"]:::terminal
    AuthIssues["Complete(AuthIssues)"]:::terminal
    GetAuthState --> |"GetAuthStateSuccess(NotAuthenticated)"| NotAuthenticated:::terminal
    GetAuthState --> |"GetAuthStateSuccess(AuthIssues)"| AuthIssues:::terminal
    GetAuthState --> |"GetAuthStateSuccess(Authenticated)"| EnsureCapabilities
    EnsureCapabilities --> |EnsureCapabilitiesSuccess| Authenticated:::terminal
    EnsureCapabilities --> |Error| AuthIssues:::terminal

    classDef default fill:#0af, color:black, stroke:black
    classDef terminal fill:#FC766A, stroke: black;
```
