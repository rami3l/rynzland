----------------------------- MODULE RustupGC -----------------------------

EXTENDS Naturals, FiniteSets

Refs == {"stable", "nightly"}
Hashes == {1, 2, 3}
Transactions == {"tx1", "tx2", "tx3"}

Incarnations ==
    {[hash |-> 1, id |-> 1],
     [hash |-> 1, id |-> 2],
     [hash |-> 1, id |-> 3],
     [hash |-> 2, id |-> 1],
     [hash |-> 2, id |-> 2],
     [hash |-> 3, id |-> 1],
     [hash |-> 3, id |-> 2]}

VARIABLES
    heap, trash, refs, target, pending, phase,
    lockOwner, lockedHash

vars ==
    <<heap, trash, refs, target, pending, phase,
      lockOwner, lockedHash>>

TypeOK ==
    /\ heap \subseteq Incarnations
    /\ trash \subseteq Incarnations
    /\ heap \cap trash = {}
    /\ refs \in [Refs -> (Hashes \cup {0})]
    /\ target \in [Transactions -> (Refs \cup {"none"})]
    /\ pending \in [Transactions -> (Hashes \cup {0})]
    /\ phase \in [Transactions -> {"idle", "running"}]
    /\ lockOwner \in Transactions \cup {"none", "gc"}
    /\ lockedHash \in Hashes \cup {0}

HashOf(i) == i.hash

HasHash(s, h) ==
    \E i \in s : HashOf(i) = h

Reachable(h) ==
    \E r \in Refs : refs[r] = h

FreshIncarnation(h) ==
    CHOOSE i \in Incarnations :
        /\ i.hash = h
        /\ i \notin heap
        /\ i \notin trash

Init ==
    /\ heap = {}
    /\ trash = {}
    /\ refs = [r \in Refs |-> 0]
    /\ target = [t \in Transactions |-> "none"]
    /\ pending = [t \in Transactions |-> 0]
    /\ phase = [t \in Transactions |-> "idle"]
    /\ lockOwner = "none"
    /\ lockedHash = 0

(***************************************************************************)
(* Transaction                                                             *)
(***************************************************************************)

Start(t, r, h) ==
    /\ phase[t] = "idle"
    /\ r \in Refs
    /\ h \in Hashes
    /\ target' = [target EXCEPT ![t] = r]
    /\ pending' = [pending EXCEPT ![t] = h]
    /\ phase' = [phase EXCEPT ![t] = "running"]
    /\ UNCHANGED <<heap, trash, refs, lockOwner, lockedHash>>

AcquireTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = "none"
    /\ lockOwner' = t
    /\ lockedHash' = pending[t]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

CreateOrReuse(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ IF HasHash(heap, pending[t])
          THEN heap' = heap
          ELSE heap' = heap \cup {FreshIncarnation(pending[t])}
    /\ UNCHANGED <<trash, refs, target, pending, phase, lockOwner, lockedHash>>

Publish(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ lockedHash = pending[t]
    /\ HasHash(heap, pending[t])
    /\ refs' = [refs EXCEPT ![target[t]] = pending[t]]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, target, pending>>

CrashTx(t) ==
    /\ phase[t] = "running"
    /\ lockOwner = t
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, refs, target, pending>>

(***************************************************************************)
(* Garbage collection                                                      *)
(***************************************************************************)

AcquireGC(h) ==
    /\ h \in Hashes
    /\ lockOwner = "none"
    /\ lockOwner' = "gc"
    /\ lockedHash' = h
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

Reclaim ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ ~Reachable(lockedHash)
    /\ HasHash(heap, lockedHash)
    /\ LET i == CHOOSE x \in heap : HashOf(x) = lockedHash
       IN /\ heap' = heap \ {i}
          /\ trash' = trash \cup {i}
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<refs, target, pending, phase>>

SkipGC ==
    /\ lockOwner = "gc"
    /\ lockedHash \in Hashes
    /\ Reachable(lockedHash)
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

CrashGC ==
    /\ lockOwner = "gc"
    /\ lockOwner' = "none"
    /\ lockedHash' = 0
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

(***************************************************************************)
(* Next                                                                    *)
(***************************************************************************)

Next ==
    \/ \E t \in Transactions, r \in Refs, h \in Hashes :
           Start(t, r, h)
    \/ \E t \in Transactions : AcquireTx(t)
    \/ \E t \in Transactions : CreateOrReuse(t)
    \/ \E t \in Transactions : Publish(t)
    \/ \E t \in Transactions : CrashTx(t)
    \/ \E h \in Hashes : AcquireGC(h)
    \/ Reclaim
    \/ SkipGC
    \/ CrashGC

Spec ==
    Init /\ [][Next]_vars

(***************************************************************************)
(* Safety properties                                                       *)
(***************************************************************************)

RefIntegrity ==
    \A r \in Refs :
        refs[r] = 0 \/ HasHash(heap, refs[r])

HeapTrashDisjoint ==
    heap \cap trash = {}

LockConsistency ==
    /\ lockOwner = "none" => lockedHash = 0
    /\ lockOwner # "none" => lockedHash \in Hashes

TxLockConsistency ==
    \A t \in Transactions :
        lockOwner = t => phase[t] = "running"

(***************************************************************************)
(* Invariants                                                              *)
(***************************************************************************)

Invariant ==
    /\ TypeOK
    /\ RefIntegrity
    /\ HeapTrashDisjoint
    /\ LockConsistency
    /\ TxLockConsistency

=============================================================================
