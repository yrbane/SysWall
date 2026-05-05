# syswall-daemon

## Smoke test NFQUEUE

```bash
sudo SYSWALL_TEST_NFQUEUE=1 cargo test -p syswall-daemon --test nfqueue_smoke_test -- --nocapture
```

Requiert `CAP_NET_ADMIN` (ou root). Le test ouvre une queue, attend 100 ms, déclenche le cancel,
et vérifie que la boucle termine sans panic.

Pour exercer le flux complet (avec règles nft) :
1. Lancer le démon en root : `sudo cargo run --bin syswall-daemon`
2. `dmesg | grep nfnetlink_queue` pour s'assurer que le module kernel est chargé.
3. `sudo nft list ruleset | grep queue` doit montrer la règle d'interception.
