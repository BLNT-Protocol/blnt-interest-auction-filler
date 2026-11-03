# Operator deployment records

There is no canonical BLNT auction-filler contract instance. Each independent auctioneer deploys an instance owned by its own filler account.

`scripts/deploy.sh deploy` writes an operator-specific JSON record into this directory. Those generated records are ignored by Git because they identify an operator's owner, deployer, instance, salt, and transactions. Keep the record with the bot's private operational configuration.

The committed `example.json` documents the record shape. Shared release provenance belongs in `releases/`, while canonical network bindings belong in `config/`.
