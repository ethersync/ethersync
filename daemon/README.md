## Setup

After the usual `npm install`,
to start up the daemon in a directory (say, `output`) you need a config file in `.ethersync/config`:

```
mkdir -p output/.ethersync
echo "etherwiki=https://etherwiki.blinry.org#playground" > output/.ethersync/config
```

Then you can run it in this directory as follows
```
npm run ethersync --directory=output
```

(alternatively, `cd output && npm run ethersync`)
