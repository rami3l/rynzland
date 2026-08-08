----------------------------- MODULE RustupGC -----------------------------

EXTENDS Naturals, FiniteSets

Refs == {"stable", "nightly"}
Hashes == {1, 2, 3}
Transactions == {"tx1", "tx2", "tx3"}
Actors == Transactions \cup {"gc"}

Incarnations ==
    {[hash |-> 1, id |-> 1],
     [hash |-> 1, id |-> 2],
     [hash |-> 1, id |-> 3],
     [hash |-> 2, id |-> 1],
     [hash |-> 2, id |-> 2],
     [hash |-> 3, id |-> 1],
     [hash |-> 3, id |-> 2]}

VARIABLES
    heap, trash, refs, target, pending, phase, lockOwner

vars ==
    <<heap, trash, refs, target, pending, phase, lockOwner>>

TypeOK ==
    /\ heap \subseteq Incarnations
    /\ trash \subseteq Incarnations
    /\ heap \cap trash = {}
    /\ refs \in [Refs -> (Hashes \cup {0})]
    /\ target \in [Transactions -> (Refs \cup {"none"})]
    /\ pending \in [Transactions -> (Hashes \cup {0})]
    /\ phase \in [Transactions -> {"idle", "running"}]
    /\ lockOwner \in [Hashes -> (Actors \cup {"none"})]

HasHash(s, h) ==
    \E i \in s : i.hash = h

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
    /\ lockOwner = [h \in Hashes |-> "none"]

(***************************************************************************)
(* Transaction                                                             *)
(***************************************************************************)

Start(t, r, h) ==
    /\ phase[t] = "idle"
    /\ r \in Refs
    /\ h \in Hashes
    /\ phase' = [phase EXCEPT ![t] = "running"]
    /\ target' = [target EXCEPT ![t] = r]
    /\ pending' = [pending EXCEPT ![t] = h]
    /\ UNCHANGED <<heap, trash, refs, lockOwner>>

AcquireTx(t) ==
    /\ phase[t] = "running"
    /\ pending[t] \in Hashes
    /\ lockOwner[pending[t]] = "none"
    /\ lockOwner' = [lockOwner EXCEPT ![pending[t]] = t]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

Create(t) ==
    /\ phase[t] = "running"
    /\ pending[t] \in Hashes
    /\ lockOwner[pending[t]] = t
    /\ ~HasHash(heap, pending[t])
    /\ heap' = heap \cup {FreshIncarnation(pending[t])}
    /\ UNCHANGED <<trash, refs, target, pending, phase, lockOwner>>

Publish(t) ==
    /\ phase[t] = "running"
    /\ pending[t] \in Hashes
    /\ lockOwner[pending[t]] = t
    /\ HasHash(heap, pending[t])
    /\ refs' = [refs EXCEPT ![target[t]] = pending[t]]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ target' = [target EXCEPT ![t] = "none"]
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ lockOwner' = [lockOwner EXCEPT ![pending[t]] = "none"]
    /\ UNCHANGED <<heap, trash>>

CrashTx(t) ==
    /\ phase[t] = "running"
    /\ IF pending[t] \in Hashes /\ lockOwner[pending[t]] = t
          THEN lockOwner' =
                   [lockOwner EXCEPT ![pending[t]] = "none"]
          ELSE UNCHANGED lockOwner
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ target' = [target EXCEPT ![t] = "none"]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ UNCHANGED <<heap, trash, refs>>

(***************************************************************************)
(* Garbage collection                                                      *)
(***************************************************************************)

AcquireGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "none"
    /\ HasHash(heap, h)
    /\ lockOwner' = [lockOwner EXCEPT ![h] = "gc"]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

CollectGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "gc"
    /\ HasHash(heap, h)
    /\ ~Reachable(h)
    /\ LET i == CHOOSE x \in heap : x.hash = h
       IN /\ heap' = heap \ {i}
          /\ trash' = trash \cup {i}
    /\ lockOwner' = [lockOwner EXCEPT ![h] = "none"]
    /\ UNCHANGED <<refs, target, pending, phase>>

SkipGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "gc"
    /\ HasHash(heap, h)
    /\ Reachable(h)
    /\ lockOwner' = [lockOwner EXCEPT ![h] = "none"]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

CrashGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "gc"
    /\ lockOwner' = [lockOwner EXCEPT ![h] = "none"]
    /\ UNCHANGED <<heap, trash, refs, target, pending, phase>>

(***************************************************************************)
(* Next                                                                    *)
(***************************************************************************)

Next ==
    \/ \E t \in Transactions, r \in Refs, h \in Hashes :
          Start(t, r, h)
    \/ \E t \in Transactions : AcquireTx(t)
    \/ \E t \in Transactions : Create(t)
    \/ \E t \in Transactions : Publish(t)
    \/ \E t \in Transactions : CrashTx(t)
    \/ \E h \in Hashes : AcquireGC(h)
    \/ \E h \in Hashes : CrashGC(h)

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
    \A h \in Hashes :
        lockOwner[h] = "none" \/ lockOwner[h] \in Actors

TxLockConsistency ==
    \A t \in Transactions :
        \A h \in Hashes :
            lockOwner[h] = t =>
                /\ phase[t] = "running"
                /\ pending[t] = h

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
