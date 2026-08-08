----------------------------- MODULE RustupGC -----------------------------

EXTENDS Naturals

Hashes == {1, 2, 3}

Incarnations ==
    {[hash |-> 1, id |-> 1],
     [hash |-> 1, id |-> 2],
     [hash |-> 2, id |-> 1],
     [hash |-> 2, id |-> 2],
     [hash |-> 3, id |-> 1],
     [hash |-> 3, id |-> 2]}

Transactions == {"tx1", "tx2"}

VARIABLES
    heap,
    trash,
    ref,
    pending,
    phase,
    lockOwner,
    lockedHash

vars == <<heap, trash, ref, pending, phase, lockOwner, lockedHash>>

HashOf(i) == i.hash

HasHash(s, h) ==
    \E i \in s : HashOf(i) = h

FreshFor(h) ==
    {i \in Incarnations :
        HashOf(i) = h /\ i \notin heap /\ i \notin trash}

Init ==
    /\ heap = {}
    /\ trash = {}
    /\ ref = 0
    /\ pending = [t \in Transactions |-> 0]
    /\ phase = [t \in Transactions |-> "idle"]
    /\ lockOwner = "none"
    /\ lockedHash = 0

Start(t, h) ==
    /\ phase[t] = "idle"
    /\ h \in Hashes
    /\ phase' = [phase EXCEPT ![t] = "running"]
    /\ pending' = [pending EXCEPT ![t] = h]
    /\ UNCHANGED <<heap, trash, ref, lockOwner, lockedHash>>

AcquireTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = "none"
    /\ lockOwner' = t
    /\ lockedHash' = pending[t]
    /\ UNCHANGED <<heap, trash, ref, pending, phase>>

UseExisting(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ HasHash(heap, pending[t])
    /\ UNCHANGED <<heap, trash, ref, pending, phase, lockOwner, lockedHash>>

CreateNew(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ ~HasHash(heap, pending[t])
    /\ FreshFor(pending[t]) # {}
    /\ LET i == CHOOSE x \in FreshFor(pending[t]) : TRUE
       IN heap' = heap \cup {i}
    /\ UNCHANGED <<trash, ref, pending, phase, lockOwner, lockedHash>>

Publish(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ HasHash(heap, pending[t])
    /\ ref' = pending[t]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash>>

ReleaseTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, ref, pending>>

AcquireGC(h) ==
    /\ lockOwner = "none"
    /\ h \in Hashes
    /\ HasHash(heap, h)
    /\ lockOwner' = "gc"
    /\ lockedHash' = h
    /\ UNCHANGED <<heap, trash, ref, pending, phase>>

RecheckAndMoveGC ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ HasHash(heap, lockedHash)
    /\ lockedHash # ref
    /\ LET i == CHOOSE x \in heap : HashOf(x) = lockedHash
       IN /\ heap' = heap \ {i}
          /\ trash' = trash \cup {i}
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<ref, pending, phase>>

SkipGC ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ lockedHash = ref
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, ref, pending, phase>>

DeleteTrash(i) ==
    /\ i \in trash
    /\ trash' = trash \ {i}
    /\ UNCHANGED <<heap, ref, pending, phase, lockOwner, lockedHash>>

Next ==
    \/ \E t \in Transactions, h \in Hashes : Start(t, h)
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
    ref = 0 \/ HasHash(heap, ref)

LockConsistency ==
    /\ lockOwner = "none" => lockedHash = 0
    /\ lockOwner # "none" => lockedHash \in Hashes

HeapTrashDisjoint ==
    heap \cap trash = {}

NoPublishedObjectInTrash ==
    ref = 0 \/ HasHash(heap, ref)

=============================================================================
