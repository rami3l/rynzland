----------------------------- MODULE RustupGC -----------------------------

EXTENDS Naturals

Refs == {"stable", "nightly"}
Hashes == {1, 2}
Transactions == {"tx1", "tx2"}

Incarnations ==
    {[hash |-> 1, id |-> 1],
     [hash |-> 1, id |-> 2],
     [hash |-> 2, id |-> 1],
     [hash |-> 2, id |-> 2]}

VARIABLES
    heap,
    trash,
    refs,
    target,
    pending,
    phase,
    lockOwner,
    lockedHash,
    gcChecked,
    trashLocked

vars ==
    <<heap, trash, refs, target, pending, phase,
      lockOwner, lockedHash, gcChecked, trashLocked>>

HashOf(i) == i.hash

HasHash(s, h) ==
    \E i \in s : HashOf(i) = h

FreshFor(h) ==
    {i \in Incarnations :
        HashOf(i) = h /\ i \notin heap /\ i \notin trash}

Reachable(h) ==
    \E r \in Refs : refs[r] = h

Init ==
    /\ heap = {}
    /\ trash = {}
    /\ refs = [r \in Refs |-> 0]
    /\ target = [t \in Transactions |-> 0]
    /\ pending = [t \in Transactions |-> 0]
    /\ phase = [t \in Transactions |-> "idle"]
    /\ lockOwner = "none"
    /\ lockedHash = 0
    /\ gcChecked = FALSE
    /\ trashLocked = 0

Start(t, r, h) ==
    /\ phase[t] = "idle"
    /\ r \in Refs
    /\ h \in Hashes
    /\ phase' = [phase EXCEPT ![t] = "running"]
    /\ target' = [target EXCEPT ![t] = r]
    /\ pending' = [pending EXCEPT ![t] = h]
    /\ UNCHANGED <<heap, trash, refs, lockOwner, lockedHash,
                   gcChecked, trashLocked>>

AcquireTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = "none"
    /\ trashLocked = 0
    /\ lockOwner' = t
    /\ lockedHash' = pending[t]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase,
                   gcChecked, trashLocked>>

UseExisting(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ HasHash(heap, pending[t])
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase,
                   lockOwner, lockedHash, gcChecked, trashLocked>>

CreateNew(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ ~HasHash(heap, pending[t])
    /\ FreshFor(pending[t]) # {}
    /\ LET i == CHOOSE x \in FreshFor(pending[t]) : TRUE
       IN heap' = heap \cup {i}
    /\ UNCHANGED <<trash, refs, target, pending, phase,
                   lockOwner, lockedHash, gcChecked, trashLocked>>

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
    /\ UNCHANGED <<heap, trash, gcChecked, trashLocked>>

ReleaseTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ target' = [target EXCEPT ![t] = 0]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, refs, gcChecked, trashLocked>>

AcquireGC(h) ==
    /\ lockOwner = "none"
    /\ trashLocked = 0
    /\ h \in Hashes
    /\ HasHash(heap, h)
    /\ lockOwner' = "gc"
    /\ lockedHash' = h
    /\ gcChecked' = FALSE
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase, trashLocked>>

CheckGC ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ HasHash(heap, lockedHash)
    /\ gcChecked' = TRUE
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase,
                   lockOwner, lockedHash, trashLocked>>

RenameToTrash ==
    /\ lockOwner = "gc"
    /\ gcChecked = TRUE
    /\ lockedHash \in Hashes
    /\ HasHash(heap, lockedHash)
    /\ ~Reachable(lockedHash)
    /\ LET i == CHOOSE x \in heap : HashOf(x) = lockedHash
       IN /\ heap' = heap \ {i}
          /\ trash' = trash \cup {i}
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ gcChecked' = FALSE
    /\ UNCHANGED <<refs, target, pending, phase, trashLocked>>

CancelGC ==
    /\ lockOwner = "gc"
    /\ gcChecked = TRUE
    /\ lockedHash \in Hashes
    /\ Reachable(lockedHash)
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ gcChecked' = FALSE
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase, trashLocked>>

AcquireTrashGC(h) ==
    /\ lockOwner = "none"
    /\ trashLocked = 0
    /\ h \in Hashes
    /\ \E i \in trash : HashOf(i) = h
    /\ trashLocked' = h
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase,
                   lockOwner, lockedHash, gcChecked>>

DeleteTrash(h) ==
    /\ trashLocked = h
    /\ \A r \in Refs : refs[r] # h
    /\ LET candidates ==
              {i \in trash : HashOf(i) = h}
       IN /\ trash' = trash \ candidates
    /\ trashLocked' = 0
    /\ UNCHANGED <<heap, refs, target, pending, phase,
                   lockOwner, lockedHash, gcChecked>>

SkipTrash ==
    /\ trashLocked \in Hashes
    /\ Reachable(trashLocked)
    /\ trashLocked' = 0
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase,
                   lockOwner, lockedHash, gcChecked>>

CrashTx(t) ==
    /\ phase[t] = "running"
    /\ IF lockOwner = t
          THEN /\ lockOwner' = "none"
               /\ lockedHash' = 0
          ELSE /\ UNCHANGED <<lockOwner, lockedHash>>
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ target' = [target EXCEPT ![t] = 0]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ UNCHANGED <<heap, trash, refs, gcChecked, trashLocked>>

CrashGC ==
    /\ lockOwner = "gc"
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ gcChecked' = FALSE
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase, trashLocked>>

CrashTrashGC ==
    /\ trashLocked \in Hashes
    /\ trashLocked' = 0
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase,
                   lockOwner, lockedHash, gcChecked>>

Next ==
    \/ \E t \in Transactions, r \in Refs, h \in Hashes :
          Start(t, r, h)
    \/ \E t \in Transactions : AcquireTx(t)
    \/ \E t \in Transactions : UseExisting(t)
    \/ \E t \in Transactions : CreateNew(t)
    \/ \E t \in Transactions : Publish(t)
    \/ \E t \in Transactions : ReleaseTx(t)
    \/ \E t \in Transactions : CrashTx(t)
    \/ \E h \in Hashes : AcquireGC(h)
    \/ CheckGC
    \/ RenameToTrash
    \/ CancelGC
    \/ CrashGC
    \/ \E h \in Hashes : AcquireTrashGC(h)
    \/ \E h \in Hashes : DeleteTrash(h)
    \/ SkipTrash
    \/ CrashTrashGC

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

TxLockConsistency ==
    \A t \in Transactions :
        lockOwner = t => phase[t] = "running"

GCCheckConsistency ==
    gcChecked => lockOwner = "gc"

TrashLockConsistency ==
    trashLocked # 0 => trashLocked \in Hashes

NoSimultaneousLocks ==
    /\ lockOwner # "none" => trashLocked = 0
    /\ trashLocked # 0 => lockOwner = "none"

Invariant ==
    /\ RefIntegrity
    /\ TrashObjectsAreNotLive
    /\ HeapTrashDisjoint
    /\ LockConsistency
    /\ TxLockConsistency
    /\ GCCheckConsistency
    /\ TrashLockConsistency
    /\ NoSimultaneousLocks

=============================================================================
