---------------------------- MODULE MachIPC ----------------------------
(* Formal specification of Mach IPC mechanism for synthesized OS *)

EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS
    Tasks,          \* Set of tasks
    Threads,        \* Set of threads  
    Ports,          \* Set of ports
    Messages,       \* Set of possible messages
    MaxQueueSize    \* Maximum message queue size

VARIABLES
    portRights,     \* Port rights: [Ports -> SUBSET Tasks]
    messageQueues,  \* Message queues: [Ports -> Seq(Messages)]
    threadState,    \* Thread states: [Threads -> {"ready", "blocked", "sending", "receiving"}]
    taskPorts,      \* Task port namespaces: [Tasks -> SUBSET Ports]
    kernelMessages  \* In-transit kernel messages

--------------------------------------------------------------------------------
(* Type Invariants *)

TypeInvariant ==
    /\ portRights \in [Ports -> SUBSET Tasks]
    /\ messageQueues \in [Ports -> Seq(Messages)]
    /\ threadState \in [Threads -> {"ready", "blocked", "sending", "receiving"}]
    /\ taskPorts \in [Tasks -> SUBSET Ports]
    /\ kernelMessages \subseteq Messages

--------------------------------------------------------------------------------
(* Initial State *)

Init ==
    /\ portRights = [p \in Ports |-> {}]
    /\ messageQueues = [p \in Ports |-> <<>>]
    /\ threadState = [t \in Threads |-> "ready"]
    /\ taskPorts = [t \in Tasks |-> {}]
    /\ kernelMessages = {}

--------------------------------------------------------------------------------
(* Port Operations *)

AllocatePort(task, port) ==
    /\ port \notin UNION {taskPorts[t] : t \in Tasks}  \* Port not already allocated
    /\ portRights' = [portRights EXCEPT ![port] = {task}]
    /\ taskPorts' = [taskPorts EXCEPT ![task] = taskPorts[task] \cup {port}]
    /\ UNCHANGED <<messageQueues, threadState, kernelMessages>>

DeallocatePort(task, port) ==
    /\ port \in taskPorts[task]
    /\ task \in portRights[port]
    /\ portRights' = [portRights EXCEPT ![port] = portRights[port] \ {task}]
    /\ taskPorts' = [taskPorts EXCEPT ![task] = taskPorts[task] \ {port}]
    /\ messageQueues' = [messageQueues EXCEPT ![port] = <<>>]  \* Clear queue
    /\ UNCHANGED <<threadState, kernelMessages>>

InsertRight(fromTask, toTask, port, rightType) ==
    /\ port \in taskPorts[fromTask]
    /\ fromTask \in portRights[port]
    /\ rightType \in {"SEND", "RECEIVE", "SEND_ONCE"}
    /\ portRights' = [portRights EXCEPT ![port] = portRights[port] \cup {toTask}]
    /\ taskPorts' = [taskPorts EXCEPT ![toTask] = taskPorts[toTask] \cup {port}]
    /\ UNCHANGED <<messageQueues, threadState, kernelMessages>>

--------------------------------------------------------------------------------
(* Message Operations *)

SendMessage(thread, task, port, message) ==
    /\ threadState[thread] = "ready"
    /\ port \in taskPorts[task]
    /\ task \in portRights[port]
    /\ Len(messageQueues[port]) < MaxQueueSize
    /\ threadState' = [threadState EXCEPT ![thread] = "sending"]
    /\ kernelMessages' = kernelMessages \cup {message}
    /\ UNCHANGED <<portRights, messageQueues, taskPorts>>

CompleteS
end(thread, port, message) ==
    /\ threadState[thread] = "sending"
    /\ message \in kernelMessages
    /\ messageQueues' = [messageQueues EXCEPT ![port] = Append(messageQueues[port], message)]
    /\ kernelMessages' = kernelMessages \ {message}
    /\ threadState' = [threadState EXCEPT ![thread] = "ready"]
    /\ UNCHANGED <<portRights, taskPorts>>

ReceiveMessage(thread, task, port) ==
    /\ threadState[thread] = "ready"
    /\ port \in taskPorts[task]
    /\ task \in portRights[port]
    /\ Len(messageQueues[port]) > 0  \* Message available
    /\ threadState' = [threadState EXCEPT ![thread] = "receiving"]
    /\ UNCHANGED <<portRights, messageQueues, taskPorts, kernelMessages>>

CompleteReceive(thread, port) ==
    /\ threadState[thread] = "receiving"
    /\ Len(messageQueues[port]) > 0
    /\ messageQueues' = [messageQueues EXCEPT ![port] = Tail(messageQueues[port])]
    /\ threadState' = [threadState EXCEPT ![thread] = "ready"]
    /\ UNCHANGED <<portRights, taskPorts, kernelMessages>>

BlockOnReceive(thread, port) ==
    /\ threadState[thread] = "ready"
    /\ Len(messageQueues[port]) = 0  \* No message available
    /\ threadState' = [threadState EXCEPT ![thread] = "blocked"]
    /\ UNCHANGED <<portRights, messageQueues, taskPorts, kernelMessages>>

WakeupBlocked(thread, port) ==
    /\ threadState[thread] = "blocked"
    /\ Len(messageQueues[port]) > 0  \* Message now available
    /\ threadState' = [threadState EXCEPT ![thread] = "receiving"]
    /\ UNCHANGED <<portRights, messageQueues, taskPorts, kernelMessages>>

--------------------------------------------------------------------------------
(* Next State Relation *)

Next ==
    \/ \E task \in Tasks, port \in Ports:
        \/ AllocatePort(task, port)
        \/ DeallocatePort(task, port)
    \/ \E fromTask, toTask \in Tasks, port \in Ports, rightType \in {"SEND", "RECEIVE", "SEND_ONCE"}:
        InsertRight(fromTask, toTask, port, rightType)
    \/ \E thread \in Threads, task \in Tasks, port \in Ports, message \in Messages:
        \/ SendMessage(thread, task, port, message)
        \/ CompleteSend(thread, port, message)
        \/ ReceiveMessage(thread, task, port)
        \/ CompleteReceive(thread, port)
        \/ BlockOnReceive(thread, port)
        \/ WakeupBlocked(thread, port)

Spec == Init /\ [][Next]_<<portRights, messageQueues, threadState, taskPorts, kernelMessages>>

--------------------------------------------------------------------------------
(* Safety Properties *)

NoOrphanMessages ==
    \* Messages in queues must be for allocated ports
    \A port \in Ports:
        Len(messageQueues[port]) > 0 => portRights[port] # {}

NoDeadlock ==
    \* At least one thread can make progress
    \E thread \in Threads:
        threadState[thread] = "ready" \/
        (threadState[thread] = "blocked" /\ 
         \E port \in Ports: Len(messageQueues[port]) > 0)

BoundedQueues ==
    \* All message queues respect size limit
    \A port \in Ports:
        Len(messageQueues[port]) <= MaxQueueSize

PortRightsConsistency ==
    \* Port rights are consistent with task namespaces
    \A port \in Ports, task \in Tasks:
        (port \in taskPorts[task]) => (task \in portRights[port])

--------------------------------------------------------------------------------
(* Liveness Properties *)

MessageEventuallyDelivered ==
    \* Every sent message is eventually delivered
    \A message \in Messages:
        (message \in kernelMessages) ~>
        (\E port \in Ports: message \in Range(messageQueues[port]))

BlockedThreadsEventuallyWake ==
    \* Blocked threads eventually wake when messages arrive
    \A thread \in Threads:
        (threadState[thread] = "blocked") ~>
        (threadState[thread] # "blocked")

--------------------------------------------------------------------------------
(* Theorem: Main Safety *)

THEOREM Safety == Spec => [](TypeInvariant /\ NoOrphanMessages /\ 
                             BoundedQueues /\ PortRightsConsistency)

THEOREM Liveness == Spec => (MessageEventuallyDelivered /\ 
                             BlockedThreadsEventuallyWake)

================================================================================