# Simulation

Demonstrates running kora as a simulation.

REVM-based example chain driven by threshold-simplex.

This example uses `alloy-evm` as the integration layer
above `revm` and keeps the execution backend generic over
the database trait boundary (`Database` + `DatabaseCommit`).

