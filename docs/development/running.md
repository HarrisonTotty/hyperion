# Running the server and the client

The [quick start](../../README.md#quick-start) runs both with their defaults. This is how to see a
generated system in `VIEW`, and every option of the server and of the client.

## Seeing a generated system in `VIEW`

Until sessions exist, the ship is a stand-in that the server starts at the galactic centre, in no
system, and no console moves it. So a plain `just server` and `just client` show `VIEW`'s kept
test scene under `TRAINING`, with `SCENE NOT AVAILABLE: the ship is in no system`. To see the
server's scene of a real system:

```sh
just server        # in one terminal
just place-ship    # in another, once the server is up
just client        # then F2, OPEN the universe place-ship names, and F4 for VIEW
```

`just place-ship` is a client of the running server, like the bridge. It opens the universe
`Dev Fixture` (creating it with seed `4d2` the first time), picks the system nearest
`0,26000,0` ly that has a planet, and sends `scene_ship` to put the ship 0.01 au behind that
planet along its orbit, at rest in the planet's frame, with the clock at 0 s running at 1×. The
seat camera, `VIEW`'s default, looks along the ship's nose, which for the stand-in is its velocity
in the system frame, so the planet is straight ahead. Options choose the rest
(`just place-ship --help`):

| Option                       | Default                                 | What                                                       |
| ---------------------------- | --------------------------------------- | ---------------------------------------------------------- |
| `--universe <NAME or ID>`    | `Dev Fixture`                           | Universe; a name not found is created with `--seed`        |
| `--seed <HEX>`               | `4d2`                                   | Seed of a universe it creates                              |
| `--system <ID or DESIG>`     | nearest with a planet                   | System, by ID, or by designation within the search         |
| `--near <X,Y,Z>`, `--radius` | `0,26000,0` ly, `40` ly                 | Where the system is searched for                           |
| `--look-at <TARGET>`         | `planet`                                | `planet` (the first), `barycentre`, or a body ID           |
| `--distance <AU>`            | 0.01 from a body, 1 from the barycentre | Distance from the target                                   |
| `--time <S>`, `--rate <N>`   | `0`, `1`                                | Scene clock: time from the epoch, and 0 or 1, 10 … 100,000 |
| `--address`, `--port`        | as `just client`                        | The server, also from `HYPERION_SERVER_ADDR` and `_PORT`   |

`--look-at barycentre` puts the ship at rest 1 au along galactic +z from the barycentre, looking
down at it: a single system's star, but empty space between the stars of a wide multiple. The
setting is not saved, so run `just place-ship` again after each server start, and it moves the
scene of every client of that universe.

## Server configuration

The server takes these options, each of which can instead be set by its environment variable.
An option given on the command line wins over its variable.

| Option                  | Variable                       | Default                                                | What                                         |
| ----------------------- | ------------------------------ | ------------------------------------------------------ | -------------------------------------------- |
| `--address`             | `HYPERION_ADDR`                | `127.0.0.1`                                            | IP address to listen on                      |
| `--port`                | `HYPERION_PORT`                | `7878`                                                 | Port to listen on                            |
| `--data-dir`            | `HYPERION_DATA_DIR`            | `./hyperion-data`                                      | Where universes are saved                    |
| `--num-workers`         | `HYPERION_WORKERS`             | available parallelism less one, at least one           | Generation worker threads                    |
| `--cell-cache`          | `HYPERION_CELL_CACHE_MB`       | `256`                                                  | Cache of generated cells, in MiB             |
| `--map-cache`           | `HYPERION_MAP_CACHE_MB`        | `64`                                                   | Cache of galaxy density maps, in MiB         |
| `--system-cache`        | `HYPERION_SYSTEM_CACHE_MB`     | `128`                                                  | Cache of generated systems' stars, in MiB    |
| `--body-cache`          | `HYPERION_BODY_CACHE_MB`       | `128`                                                  | Cache of planetary systems and events in MiB |
| `--brief-cache`         | `HYPERION_BRIEF_CACHE_MB`      | `64`                                                   | Cache of range briefs' star models, in MiB   |
| `--sky-cache`           | `HYPERION_SKY_CACHE_MB`        | `0`, off                                               | Cache of the sky census's cells, in MiB      |
| `--sky-tables`          | `HYPERION_SKY_TABLES_MB`       | `160`                                                  | Cache of each galaxy's sky tables, in MiB    |
| `--surface-cache`       | `HYPERION_SURFACE_CACHE_MB`    | `128`                                                  | Cache of bodies' surface fields, in MiB      |
| `--surface-passes`      | `HYPERION_SURFACE_PASSES`      | the lesser of 4 and the workers less one, at least one | Coarse surface passes run at once            |
| `--serve-sky`           | `HYPERION_SERVE_SKY`           | on                                                     | Serve `sky` requests                         |
| `--keep-skies`          | `HYPERION_KEEP_SKIES`          | on                                                     | Answer a sky asked again from memory         |
| `--kept-skies-disk`     | `HYPERION_KEPT_SKIES_DISK_MB`  | `256`                                                  | The kept skies on disk, in MiB               |
| `--hold-kept-skies`     | `HYPERION_HOLD_KEPT_SKIES`     | on                                                     | Answer a later sky from a kept one           |
| `--sky-qos`             | `HYPERION_SKY_QOS`             | `background` on macOS, `utility` elsewhere             | A sky's class after its first reply          |
| `--stop-on-stdin-close` | `HYPERION_STOP_ON_STDIN_CLOSE` | off                                                    | Stop gracefully when standard input closes   |

On Apple silicon `--num-workers` defaults to no more than its performance threads with a third of
its efficiency threads, less two (four on an M5's 4 performance and 6 efficiency cores), so that
the pool and a client's height workers together stay within about twice the performance cores.

`--stop-on-stdin-close` is for a server run as another program's child. The parent stops it by
closing its standard input, which on Windows is the only graceful stop a parent has, and if the
parent dies the pipe closes, so the server never outlives it. It is off by default: with it, a
server started from a terminal stops at an end of input typed there (Ctrl-D on Unix, Ctrl-Z and
Enter on Windows), and one started with standard input closed, as systemd starts it, stops at once.
Its variable takes `y`, `yes`, `t`, `true`, `on` or `1`, or `n`, `no`, `f`, `false`, `off` or `0`,
in any case, and refuses anything else.

`--serve-sky` serves the `sky` request, the stars, band and limits a view's sky is drawn from.
It is on by default (since rendering plan R13's R13.T2, the interim sky). `--serve-sky=false`, or
the variable set to a false value, turns it off: the server then answers `sky` as `unsupported`,
as before the sky was served, and opening a universe builds no sky tables in the background. Its
variable takes the same values as `--stop-on-stdin-close`'s, and so does the option after an equals
sign (`--serve-sky=off`); given alone, `--serve-sky` is on.

`--sky-cache` is 0 by default, which turns the sky's cell cache off: each sky's census reads no
cache and keeps no cell. For now a sky served from the cache, asked again or after a jump, costs
about what it costs without it, since its census still generates every star system, so the cache
would hold up to 1,280 MiB for no saving a player would feel (rendering plan R06, R06.T8.o).
`1280` is the size that holds a sky near the Sun and the cells a 1,000 ly jump adds, the value to
set for a warm sky. The default returns to it once a sky asked again costs at most half of the
first.

`--keep-skies` keeps the final replies of the last skies finished, up to 16 MiB, about two skies,
and answers a sky asked again exactly as one of them from memory: a relaunched client's or a
reopened view's sky costs next to nothing, while a sky asked from anywhere else is computed as
before (rendering plan R06, R06.T11.l). `--keep-skies=false` computes every sky afresh. It takes
the same values as `--serve-sky`.

`--kept-skies-disk` keeps the same final replies on disk too, up to 256 MiB, some 33 skies at the
most stars a sky lists, in `skies/` under the data directory, so that a sky asked again after a
restart is answered from its file (R06.T11.n). The least recently used go first. A file that is
torn, or was written by another generator or protocol version or under other sky settings, is
never read; `0` keeps none on disk, and `--keep-skies=false` none anywhere.

`--hold-kept-skies` answers a sky asked at a later time, or from a little way off, with a kept
reply of the same request otherwise that still holds there: asked before its `valid_until`, and
from within a tenth of a pixel of its nearest star's parallax at 1080p across 60°, as the client
holds a sky it has (R06.T11.p). The reply goes as it was kept, its own time and observer
unchanged, so a relaunched client with the clock running costs no census. Skies written before
it are named otherwise, and are never read again; they go first as new skies are written.
`--hold-kept-skies=false` answers only a sky asked exactly again. It takes the same values as
`--serve-sky`.

`--sky-qos` is the class a sky's census runs at after its first reply: `background`, the default on
macOS, runs it at the quality-of-service class background, which Apple silicon keeps to its
efficiency cores, and `utility` at the class the pool's bulk work runs at otherwise (R06.T11.o).
Elsewhere either value leaves the pool's bulk work at its lowered priority, nice 10 on Linux.

The data directory is created with the first universe. Each universe is one directory,
`universes/<id>/`, holding a small `universe.json` with its name, seed and generator version;
nothing generated is saved but the kept skies, in `skies/`, and the streams' tube envelopes, in
`envelopes/` (at most 16 MiB, so that a relaunched server's first range query builds only the tube
tables it reaches), either of which can be removed at any time. To delete a universe, stop the
server and remove its directory.

## Client configuration

The client takes the server to link to, as an option or as its variable. It links to
`ws://<address>:<port>/ws`, so by default a server on this machine.

| Option      | Variable               | Default     | What                                  |
| ----------- | ---------------------- | ----------- | ------------------------------------- |
| `--address` | `HYPERION_SERVER_ADDR` | `127.0.0.1` | IP address or host name of the server |
| `--port`    | `HYPERION_SERVER_PORT` | `7878`      | Port the server listens on            |

The variables name the server, not the client, so they are deliberately not the server's own
`HYPERION_ADDR` and `HYPERION_PORT`: an address to listen on and an address to connect to are not
the same thing. Both options are read at launch, so no rebuild is needed to point the client
somewhere else, and the `LINK` display shows the endpoint in use.
