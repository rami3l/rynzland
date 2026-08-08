----------------------------- MODULE RustupGC -----------------------------

EXTENDS Naturals

Refs == {"stable", "nightly"}
Hashes == {1, 2}

Incarnations ==
    {[hash |-> 1, id |-> 1],
     [hash |-> 1, id |-> 2],
     [hash |-> 2, id |-> 1],
     [hash |-> 2, id |-> 2]}

Transactions == {"tx1", "tx2"}

VARIABLES
    heap,
    trash,
    refs,
    target,
    pending,
    phase,
    lockOwner,
    lockedHash

vars ==
    <<heap, trash, refs, target, pending, phase, lockOwner, lockedHash>>

HashOf(i) == i.hash

HasHash(s, h) ==
    \E i \in s : HashOf(i) = h

FreshFor(h) ==
    {i \in Incarnations :
        HashOf(i) = h /\ i \notin heap /\ i \notin trash}

Reachable ==
    {h \in Hashes :
        \E r \in Refs : refs[r] = h}

Init ==
    /\ heap = {}
    /\ trash = {}
    /\ refs = [r \in Refs |-> 0]
    /\ target = [t \in Transactions |-> 0]
    /\ pending = [t \in Transactions |-> 0]
    /\ phase = [t \in Transactions |-> "idle"]
    /\ lockOwner = "none"
    /\ lockedHash = 0

Start(t, r, h) ==
    /\ phase[t] = "idle"
    /\ r \in Refs
    /\ h \in Hashes
    /\ phase' = [phase EXCEPT ![t] = "running"]
    /\ target' = [target EXCEPT ![t] = r]
    /\ pending' = [pending EXCEPT ![t] = h]
    /\ UNCHANGED <<heap, trash, refs, lockOwner, lockedHash>>

AcquireTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = "none"
    /\ lockOwner' = t
    /\ lockedHash' = pending[t]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

UseExisting(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ HasHash(heap, pending[t])
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase, lockOwner, lockedHash>>

CreateNew(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ ~HasHash(heap, pending[t])
    /\ FreshFor(pending[t]) # {}
    /\ LET i == CHOOSE x \in FreshFor(pending[t]) : TRUE
       IN heap' = heap \cup {i}
    /\ UNCHANGED <<trash, refs, target, pending, phase, lockOwner, lockedHash>>

Publish(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ HasHash(heap, pending[t])
    /\ refs' = [refs EXCEPT ![target[t]] = pending[t]]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ target' = [target EXCEPT ![t] = 0]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash>>

ReleaseTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ target' = [target EXCEPT ![t] = 0]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, refs>>

AcquireGC(h) ==
    /\ lockOwner = "none"
    /\ h \in Hashes
    /\ HasHash(heap, h)
    /\ lockOwner' = "gc"
    /\ lockedHash' = h
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

RecheckAndMoveGC ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ HasHash(heap, lockedHash)
    /\ lockedHash \notin Reachable
    /\ LET i == CHOOSE x \in heap : HashOf(x) = lockedHash
       IN /\ heap' = heap \ {i}
          /\ trash' = trash \cup {i}
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<refs, target, pending, phase>>

SkipGC ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ lockedHash \in Reachable
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

DeleteTrash(i) ==
    /\ i \in trash
    /\ i \notin heap
    /\ trash' = trash \ {i}
    /\ UNCHANGED <<heap, refs, target, pending, phase, lockOwner, lockedHash>>

Next ==
    \/ \E t \in Transactions, r \in Refs, h \in Hashes :
          Start(t, r, h)
    \/ \E t \in Transactions : AcquireTx(t)
    \/ \E t \in Transactions : UseExisting(t)
    \/ \E t \in Transactions : CreateNew(t)
    \/ \E t \in Transactions : Publish(t)
    \/ \E t \in Transactions : ReleaseTx(t)
    \/ \E h \in Hashes : AcquireGC(h)
    \/ RecheckAndMoveGC
    \/ SkipGC
    \/ \E i \in Incarnations : DeleteTrash(i)

Spec ==
    Init /\ [][Next]_vars

RefIntegrity ==
    \A r \in Refs :
        refs[r] = 0 \/ HasHash(heap, refs[r])

TrashObjectsAreNotLive ==
    \A i \in trash :
        i \notin heap

HeapTrashDisjoint ==
    heap \cap trash = {}

LockConsistency ==
    /\ lockOwner = "none" => lockedHash = 0
    /\ lockOwner # "none" => lockedHash \in Hashes

Invariant ==
    /\ RefIntegrity
    /\ TrashObjectsAreNotLive
    /\ HeapTrashDisjoint
    /\ LockConsistency

=============================================================================
