# tuma-platform

**Template — nothing is built here yet.**

This folder is the future home of Tuma's web platform: the **admin console**
and the **merchant web app** (Master Blueprint ch 8 and 21, screen maps in
ch 38). It lands after the customer loop is real:

| Surface | Lands at | Purpose |
|---|---|---|
| Merchant web app | V1.5 | Merchant operating system: dashboard, orders, menu, store controls, finance |
| Admin console | V2.5 | Platform control plane: live ops, orders, merchants, riders, payments, audit |

The stack for this folder is deliberately **not decided yet** — it will be
chosen when V1.5 approaches, based on what the server API (OpenAPI spec from
`tuma-server`) and the team's needs look like then.

Until then, all energy goes into `tuma-app` and `tuma-server`.
