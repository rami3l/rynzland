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

VARIABLES heap, trash, ref, pending, phase, lockOwner, lockedHash

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
    /\ pending = 0
    /\ phase = "idle"
    /\ lockOwner = "none"
    /\ lockedHash = 0

Start(h) ==
    /\ phase = "idle"
    /\ h \in Hashes
    /\ phase' = "running"
    /\ pending' = h
    /\ UNCHANGED <<heap, trash, ref, lockOwner, lockedHash>>

AcquireTx ==
    /\ phase = "running"
    /\ lockOwner = "none"
    /\ lockOwner' = "tx"
    /\ lockedHash' = pending
    /\ UNCHANGED <<heap, trash, ref, pending, phase>>

UseExisting ==
    /\ phase = "running"
    /\ lockOwner = "tx"
    /\ lockedHash = pending
    /\ HasHash(heap, pending)
    /\ UNCHANGED <<heap, trash, ref, pending, phase, lockOwner, lockedHash>>

CreateNew ==
    /\ phase = "running"
    /\ lockOwner = "tx"
    /\ lockedHash = pending
    /\ ~HasHash(heap, pending)
    /\ FreshFor(pending) # {}
    /\ LET i == CHOOSE x \in FreshFor(pending) : TRUE
       IN heap' = heap \cup {i}
    /\ UNCHANGED <<trash, ref, pending, phase, lockOwner, lockedHash>>

Publish ==
    /\ phase = "running"
    /\ lockOwner = "tx"
    /\ lockedHash = pending
    /\ HasHash(heap, pending)
    /\ ref' = pending
    /\ pending' = 0
    /\ phase' = "idle"
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash>>

ReleaseTx ==
    /\ phase = "running"
    /\ lockOwner = "tx"
    /\ lockedHash = pending
    /\ phase' = "idle"
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, ref, pending>>

AcquireGC(h) ==
    /\ phase = "idle"
    /\ lockOwner = "none"
    /\ h \in Hashes
    /\ HasHash(heap, h)
    /\ phase' = "gc"
    /\ pending' = h
    /\ lockOwner' = "gc"
    /\ lockedHash' = h
    /\ UNCHANGED <<heap, trash, ref>>

RecheckAndMoveGC ==
    /\ phase = "gc"
    /\ lockOwner = "gc"
    /\ lockedHash = pending
    /\ HasHash(heap, pending)
    /\ pending # ref
    /\ LET i == CHOOSE x \in heap : HashOf(x) = pending
       IN /\ heap' = heap \ {i}
          /\ trash' = trash \cup {i}
    /\ phase' = "idle"
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<ref, pending>>

SkipGC ==
    /\ phase = "gc"
    /\ lockOwner = "gc"
    /\ lockedHash = pending
    /\ pending = ref
    /\ phase' = "idle"
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, ref, pending>>

DeleteTrash(i) ==
    /\ i \in trash
    /\ trash' = trash \ {i}
    /\ UNCHANGED <<heap, ref, pending, phase, lockOwner, lockedHash>>

Next ==
    \/ \E h \in Hashes : Start(h)
    \/ AcquireTx
    \/ UseExisting
    \/ CreateNew
    \/ Publish
    \/ ReleaseTx
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
    ref = 0 \/ \E i \in heap : HashOf(i) = ref

=============================================================================
