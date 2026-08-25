----------------------------- MODULE RustupGC -----------------------------

EXTENDS Naturals, FiniteSets

CONSTANTS
    \* @type: Set(Str);
    Refs,
    \* @type: Set(Int);
    Hashes,
    \* @type: Set(Str);
    Transactions,
    \* @type: Set(Int);
    IncarnationIds

Actors == Transactions \cup {"gc"}

Incarnations ==
    {[hash |-> h, id |-> i]
        : h \in Hashes, i \in IncarnationIds}

VARIABLES
    \* @type: Set({hash: Int, id: Int});
    heap,
    \* @type: Str -> Int;
    refs,
    \* @type: Str -> Str;
    target,
    \* @type: Str -> Int;
    pending,
    \* @type: Str -> Str;
    phase,
    \* @type: Int -> Str;
    lockOwner

vars ==
    <<heap, refs, target, pending, phase, lockOwner>>

TypeOK ==
    /\ heap \subseteq Incarnations
    /\ refs \in [Refs -> (Hashes \cup {0})]
    /\ target \in [Transactions -> (Refs \cup {"none"})]
    /\ pending \in [Transactions -> (Hashes \cup {0})]
    /\ phase \in [Transactions -> {"idle", "running"}]
    /\ lockOwner \in [Hashes -> (Actors \cup {"none"})]

\* @type: (Set({hash: Int, id: Int}), Int) => Bool;
HasHash(s, h) ==
    \E i \in s : i.hash = h

Reachable(h) ==
    \E r \in Refs : refs[r] = h

FreshFor(h) ==
    {i \in Incarnations : i.hash = h /\ i \notin heap}

Init ==
    /\ heap = {}
    /\ refs = [r \in Refs |-> 0]
    /\ target = [t \in Transactions |-> "none"]
    /\ pending = [t \in Transactions |-> 0]
    /\ phase = [t \in Transactions |-> "idle"]
    /\ lockOwner = [h \in Hashes |-> "none"]

\****************************************************************************
\* Transaction
\****************************************************************************

Remove(r) ==
    /\ r \in Refs
    /\ refs[r] \in Hashes
    /\ refs' = [refs EXCEPT ![r] = 0]
    /\ UNCHANGED <<heap, target, pending, phase, lockOwner>>

Start(t, r, h) ==
    /\ phase[t] = "idle"
    /\ r \in Refs
    /\ h \in Hashes
    /\ phase' = [phase EXCEPT ![t] = "running"]
    /\ target' = [target EXCEPT ![t] = r]
    /\ pending' = [pending EXCEPT ![t] = h]
    /\ UNCHANGED <<heap, refs, lockOwner>>

AcquireTx(t) ==
    /\ phase[t] = "running"
    /\ pending[t] \in Hashes
    /\ lockOwner[pending[t]] = "none"
    /\ lockOwner' = [lockOwner EXCEPT ![pending[t]] = t]
    /\ UNCHANGED <<heap, refs, target, pending, phase>>

Create(t) ==
    /\ phase[t] = "running"
    /\ pending[t] \in Hashes
    /\ lockOwner[pending[t]] = t
    /\ ~HasHash(heap, pending[t])
    /\ FreshFor(pending[t]) # {}
    /\ LET i == CHOOSE x \in FreshFor(pending[t]) : TRUE
       IN heap' = heap \cup {i}
    /\ UNCHANGED <<refs, target, pending, phase, lockOwner>>

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
    /\ UNCHANGED <<heap>>

CrashTx(t) ==
    /\ phase[t] = "running"
    /\ IF pending[t] \in Hashes /\ lockOwner[pending[t]] = t
          THEN lockOwner' =
                   [lockOwner EXCEPT ![pending[t]] = "none"]
          ELSE UNCHANGED lockOwner
    /\ phase' = [phase EXCEPT ![t] = "idle"]
    /\ target' = [target EXCEPT ![t] = "none"]
    /\ pending' = [pending EXCEPT ![t] = 0]
    /\ UNCHANGED <<heap, refs>>

\***************************************************************************
\* Garbage collection
\***************************************************************************

AcquireGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "none"
    /\ HasHash(heap, h)
    /\ lockOwner' = [lockOwner EXCEPT ![h] = "gc"]
    /\ UNCHANGED <<heap, refs, target, pending, phase>>

CollectGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "gc"
    /\ HasHash(heap, h)
    /\ ~Reachable(h)
    /\ LET i == CHOOSE x \in heap : x.hash = h
       IN heap' = heap \ {i}
    /\ UNCHANGED <<refs, target, pending, phase, lockOwner>>

ReleaseOrCrashGC(h) ==
    /\ h \in Hashes
    /\ lockOwner[h] = "gc"
    /\ lockOwner' = [lockOwner EXCEPT ![h] = "none"]
    /\ UNCHANGED <<heap, refs, target, pending, phase>>

\****************************************************************************
\* Next
\****************************************************************************

Next ==
    \/ \E r \in Refs : Remove(r)
    \/ \E t \in Transactions, r \in Refs, h \in Hashes :
          Start(t, r, h)
    \/ \E t \in Transactions : AcquireTx(t)
    \/ \E t \in Transactions : Create(t)
    \/ \E t \in Transactions : Publish(t)
    \/ \E t \in Transactions : CrashTx(t)
    \/ \E h \in Hashes : AcquireGC(h)
    \/ \E h \in Hashes : CollectGC(h)
    \/ \E h \in Hashes : ReleaseOrCrashGC(h)

Spec == Init /\ [][Next]_vars

TxStep(t) ==
    \/ AcquireTx(t)
    \/ Create(t)
    \/ Publish(t)
    \/ CrashTx(t)

FairSpec ==
    /\ Spec
    \* Once a transaction can proceed, it will eventually take a step.
    /\ \A t \in Transactions : WF_vars(TxStep(t))
    \* GC cannot hold the object lock forever.
    /\ \A h \in Hashes : WF_vars(ReleaseOrCrashGC(h))
    \* GC will eventually acquire the object lock if given the opportunity infinitely often.
    /\ \A h \in Hashes : SF_vars(AcquireGC(h))
    \* GC will eventually collect the object if given the opportunity infinitely often.
    /\ \A h \in Hashes : SF_vars(CollectGC(h))

\****************************************************************************
\* Invariants
\****************************************************************************

RefIntegrity ==
    \A r \in Refs :
        refs[r] = 0 \/ HasHash(heap, refs[r])

LockConsistency ==
    \A h \in Hashes :
        lockOwner[h] = "none" \/ lockOwner[h] \in Actors

TxLockConsistency ==
    \A t \in Transactions, h \in Hashes :
        lockOwner[h] = t => phase[t] = "running" /\ pending[t] = h

\****************************************************************************
\* Properties
\****************************************************************************

TxTerminates ==
    \A t \in Transactions :
        phase[t] = "running" ~> phase[t] = "idle"

LocksEventuallyFree ==
    \A h \in Hashes :
        lockOwner[h] # "none" ~> lockOwner[h] = "none"

NoMoreUses(h) ==
    /\ ~Reachable(h)
    /\ \A t \in Transactions : pending[t] # h

QuiescentGarbageCollected ==
    \A h \in Hashes :
        (<>[] NoMoreUses(h)) => (<>[] ~HasHash(heap, h))

=============================================================================
