import 'dart:math';

import 'package:hashlib/hashlib.dart';
import 'package:player/screens/welcome/scanning.dart';
import 'package:provider/provider.dart';
import 'package:fluent_ui/fluent_ui.dart';
import 'package:material_symbols_icons/symbols.dart';
import 'package:responsive_framework/responsive_framework.dart';
import 'package:flutter_staggered_grid_view/flutter_staggered_grid_view.dart';

import '../../../utils/ax_shadow.dart';
import '../../../utils/format_time.dart';
import '../../../widgets/tile/cover_art.dart';
import '../../../widgets/gradient_container.dart';
import '../../../widgets/playback_controller/cover_wall_button.dart';
import '../../../widgets/playback_controller/constants/playback_controller_height.dart';
import '../../../messages/cover_art.pb.dart';
import '../../../providers/status.dart';

const int count = 40;

final maxHashValue = BigInt.from(1) << 64;

double stringToDouble(String input) {
  var hash = xxh3.string(input).bigInt();

  return hash / maxHashValue;
}

class PlayingTrack extends StatelessWidget {
  const PlayingTrack({super.key});

  @override
  Widget build(BuildContext context) {
    final isMini = ResponsiveBreakpoints.of(context).smallerOrEqualTo(PHONE);

    if (isMini) return const PlayingTrackMini();

    return const PlayingTrackLarge();
  }
}

class PlayingTrackMini extends StatelessWidget {
  const PlayingTrackMini({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = FluentTheme.of(context);
    final isDark = theme.brightness.isDark;
    final shadowColor = isDark ? Colors.black : theme.accentColor.lightest;

    final typography = theme.typography;

    final shadows = <Shadow>[
      Shadow(color: shadowColor, blurRadius: 12),
      Shadow(color: shadowColor, blurRadius: 24),
    ];

    final width = MediaQuery.of(context).size.width;

    return Selector<PlaybackStatusProvider,
        (String?, String?, String?, String?, double?)>(
      selector: (context, playbackStatusProvider) => (
        playbackStatusProvider.playbackStatus?.coverArtPath,
        playbackStatusProvider.playbackStatus?.artist,
        playbackStatusProvider.playbackStatus?.album,
        playbackStatusProvider.playbackStatus?.title,
        playbackStatusProvider.playbackStatus?.duration,
      ),
      builder: (context, p, child) {
        if (p.$1 == null) return Container();

        final artist = p.$2 ?? "Unknown Artist";
        final album = p.$3 ?? "Unknown Album";

        return Container(
          padding: const EdgeInsets.fromLTRB(
            48,
            48,
            48,
            playbackControllerHeight + 48,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.center,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                decoration: BoxDecoration(
                  border: Border.all(color: Colors.white, width: 4),
                  boxShadow: axShadow(9),
                ),
                child: CoverArt(
                  hint: (
                    p.$3 ?? "",
                    p.$2 ?? "",
                    'Total Time ${formatTime(p.$5 ?? 0)}'
                  ),
                  key: p.$1 != null ? Key(p.$1.toString()) : null,
                  path: p.$1,
                  size: (width - 20).clamp(0, 240),
                ),
              ),
              const SizedBox(height: 24),
              Text(
                p.$4 ?? "Unknown Track",
                style: typography.subtitle?.apply(shadows: shadows),
              ),
              const SizedBox(height: 12),
              Text(
                '$artist · $album',
                style: typography.body?.apply(shadows: shadows),
              ),
            ],
          ),
        );
      },
    );
  }
}

class PlayingTrackLarge extends StatelessWidget {
  const PlayingTrackLarge({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = FluentTheme.of(context);
    final isDark = theme.brightness.isDark;
    final shadowColor = isDark ? Colors.black : theme.accentColor.lightest;

    final shadows = <Shadow>[
      Shadow(color: shadowColor, blurRadius: 12),
      Shadow(color: shadowColor, blurRadius: 24),
    ];

    final Typography typography = theme.typography;

    return Selector<PlaybackStatusProvider,
        (String?, String?, String?, String?)>(
      selector: (context, playbackStatusProvider) => (
        playbackStatusProvider.playbackStatus?.coverArtPath,
        playbackStatusProvider.playbackStatus?.artist,
        playbackStatusProvider.playbackStatus?.album,
        playbackStatusProvider.playbackStatus?.title,
      ),
      builder: (context, p, child) {
        if (p.$1 == null) return Container();
        return Container(
          padding: const EdgeInsets.fromLTRB(
              48, 48, 48, playbackControllerHeight + 48),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            mainAxisAlignment: MainAxisAlignment.start,
            children: [
              Container(
                decoration: BoxDecoration(
                  border: Border.all(color: Colors.white, width: 4),
                  boxShadow: axShadow(9),
                ),
                child: CoverArt(
                  key: p.$1 != null ? Key(p.$1.toString()) : null,
                  path: p.$1,
                  size: 120,
                ),
              ),
              const SizedBox(width: 24),
              Column(
                mainAxisAlignment: MainAxisAlignment.end,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    p.$3 ?? "Unknown Album",
                    style: typography.bodyLarge?.apply(shadows: shadows),
                  ),
                  Text(
                    p.$4 ?? "Unknown Track",
                    style: typography.subtitle?.apply(shadows: shadows),
                  ),
                  const SizedBox(height: 12),
                  Text(
                    p.$2 ?? "Unknown Artist",
                    style: typography.body?.apply(shadows: shadows),
                  ),
                  const SizedBox(height: 28),
                ],
              ),
            ],
          ),
        );
      },
    );
  }
}

class RandomGridConfig {
  final int size;
  final double probability;

  const RandomGridConfig({required this.size, required this.probability});
}

class RandomGrid extends StatefulWidget {
  final int seed;
  final List<String> paths;
  const RandomGrid({super.key, required this.seed, required this.paths});

  @override
  RandomGridState createState() => RandomGridState();
}

class RandomGridState extends State<RandomGrid> {
  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final theme = FluentTheme.of(context);
    final isDark = theme.brightness.isDark;
    final shadowColor = isDark ? Colors.black : theme.accentColor.lightest;

    return LayoutBuilder(
      builder: (context, constraints) {
        final gridSize =
            max(max(constraints.maxWidth, constraints.maxHeight) / 24, 64);
        final crossAxisCount = (constraints.maxWidth / gridSize).ceil();
        final mainAxisCount = (constraints.maxHeight / gridSize).ceil();

        final coverArtWall = widget.paths.isEmpty
            ? Container(
                color: shadowColor,
              )
            : ClipRect(
                child: OverflowBox(
                  alignment: Alignment.topLeft,
                  maxWidth: (crossAxisCount * gridSize).toDouble(),
                  maxHeight: (mainAxisCount * gridSize).toDouble(),
                  child: Center(
                    child: GradientContainer(
                      gradientParams: GradientParams(
                        multX: 2.0,
                        multY: 2.0,
                        brightness: 1.0,
                      ),
                      effectParams: EffectParams(
                        mouseInfluence: -0.2,
                        scale: 1.25,
                        noise: 1.5,
                        bw: 0.0,
                      ),
                      color: isDark
                          ? theme.accentColor
                          : theme.accentColor.darkest,
                      color2: theme.accentColor.darkest.darken(0.7),
                      child: StaggeredGrid.count(
                        crossAxisCount: crossAxisCount,
                        mainAxisSpacing: 2,
                        crossAxisSpacing: 2,
                        children: _generateTiles(
                          crossAxisCount,
                          mainAxisCount,
                          gridSize.toDouble(),
                        ),
                      ),
                    ),
                  ),
                ),
              );

        return Stack(
          alignment: Alignment.bottomCenter,
          children: [
            Container(
              color: isDark ? null : theme.accentColor.lightest.lighten(0.2),
            ),
            coverArtWall,
            Container(
                decoration: BoxDecoration(
                    gradient: RadialGradient(
                  colors: [
                    shadowColor.withAlpha(isDark ? 20 : 140),
                    shadowColor.withAlpha(isDark ? 255 : 255),
                  ],
                  radius: 1.5,
                )),
                height: (mainAxisCount * gridSize).toDouble()),
            const PlayingTrack(),
            Container(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: const Alignment(0.0, -1.0),
                  end: const Alignment(0.0, 1.0),
                  colors: [
                    shadowColor.withAlpha(0),
                    isDark
                        ? shadowColor.withAlpha(200)
                        : shadowColor.lighten(0.2).withAlpha(220),
                  ],
                ),
              ),
              height: playbackControllerHeight,
            ),
            const Positioned(
              top: 0,
              left: 0,
              child: BackButton(),
            )
          ],
        );
      },
    );
  }

  List<Widget> _generateTiles(
      int crossAxisCount, int mainAxisCount, double gridSize) {
    List<Widget> tiles = [];
    Set<String> occupiedCells = {};

    // Step 1: Generate 4x4 tiles
    _generateTilesOfSize(
        tiles,
        occupiedCells,
        gridSize,
        [
          const RandomGridConfig(size: 4, probability: 0.2),
          const RandomGridConfig(size: 3, probability: 0.3),
          const RandomGridConfig(size: 2, probability: 0.3),
        ],
        crossAxisCount,
        mainAxisCount);
    return tiles;
  }

  void _generateTilesOfSize(
    List<Widget> tiles,
    Set<String> occupiedCells,
    double gridSize,
    List<RandomGridConfig> config,
    int crossAxisCount,
    int mainAxisCount,
  ) {
    for (int row = 0; row < mainAxisCount; row++) {
      for (int col = 0; col < crossAxisCount; col++) {
        final gridKey = '$col-$row';

        if (occupiedCells.contains(gridKey)) {
          continue;
        }

        double randomValue1 = stringToDouble('$gridKey-${widget.seed}');
        double randomValue2 = stringToDouble('$gridKey-i-${widget.seed}');
        int coverIndex = (randomValue2 * (widget.paths.length - 1)).round();

        for (var cfg in config) {
          if (randomValue1 <= cfg.probability) {
            int size = cfg.size;

            if (_canPlaceTile(
                col, row, size, crossAxisCount, mainAxisCount, occupiedCells)) {
              _markOccupiedCells(col, row, size, occupiedCells);
              tiles.add(
                StaggeredGridTile.count(
                  crossAxisCellCount: size,
                  mainAxisCellCount: size,
                  child: GridTile(
                    index: row + col * mainAxisCount,
                    row: row,
                    col: col,
                    size: size,
                    child: CoverArt(
                      path: widget.paths[coverIndex],
                      size: size * gridSize,
                    ),
                  ),
                ),
              );
              break; // Once a tile is placed, move to the next cell
            } else if (_canPlaceTile(
                col, row, 1, crossAxisCount, mainAxisCount, occupiedCells)) {
              _markOccupiedCells(col, row, 1, occupiedCells);

              tiles.add(
                StaggeredGridTile.count(
                  crossAxisCellCount: 1,
                  mainAxisCellCount: 1,
                  child: GridTile(
                      index: coverIndex,
                      row: row,
                      col: col,
                      size: 1,
                      child: CoverArt(
                        path: widget.paths[coverIndex],
                        size: 1 * gridSize,
                      )),
                ),
              );
            }
          }
        }

        if (_canPlaceTile(
            col, row, 1, crossAxisCount, mainAxisCount, occupiedCells)) {
          _markOccupiedCells(col, row, 1, occupiedCells);
          tiles.add(
            StaggeredGridTile.count(
              crossAxisCellCount: 1,
              mainAxisCellCount: 1,
              child: GridTile(
                  index: coverIndex,
                  row: row,
                  col: col,
                  size: 1,
                  child: CoverArt(
                    path: widget.paths[coverIndex],
                    size: 64.0,
                  )),
            ),
          );
        }
      }
    }
  }

  bool _canPlaceTile(int col, int row, int size, int crossAxisCount,
      int mainAxisCount, Set<String> occupiedCells) {
    for (int i = 0; i < size; i++) {
      for (int j = 0; j < size; j++) {
        if (col + i >= crossAxisCount ||
            row + j >= mainAxisCount ||
            occupiedCells.contains('${col + i}-${row + j}')) {
          return false;
        }
      }
    }
    return true;
  }

  void _markOccupiedCells(
      int col, int row, int size, Set<String> occupiedCells) {
    for (int i = 0; i < size; i++) {
      for (int j = 0; j < size; j++) {
        occupiedCells.add('${col + i}-${row + j}');
      }
    }
  }
}

class BackButton extends StatelessWidget {
  const BackButton({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final isMini = ResponsiveBreakpoints.of(context).smallerOrEqualTo(MOBILE);

    if (!isMini) return Container();

    return Padding(
      padding: const EdgeInsets.only(top: 16, left: 16),
      child: IconButton(
        icon: const Icon(
          Symbols.arrow_back,
          size: 24,
        ),
        onPressed: () {
          showCoverArtWall(context);
        },
      ),
    );
  }
}

class GridTile extends StatelessWidget {
  final int index;
  final int row;
  final int col;
  final int size;
  final Widget child;

  const GridTile(
      {super.key,
      required this.index,
      required this.row,
      required this.col,
      required this.size,
      required this.child});

  @override
  Widget build(BuildContext context) {
    return Container(
      color: FluentTheme.of(context).accentColor,
      child: Center(
        child: child,
      ),
    );
  }
}

class CoverWallView extends StatefulWidget {
  const CoverWallView({super.key});

  @override
  State<CoverWallView> createState() => _CoverWallViewState();
}

class _CoverWallViewState extends State<CoverWallView> {
  List<String> paths = [];
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();

    _fetchRandomCoverArtIds();
  }

  Future<void> _fetchRandomCoverArtIds() async {
    GetRandomCoverArtIdsRequest(count: count).sendSignalToRust();
    GetRandomCoverArtIdsResponse.rustSignalStream.listen((event) {
      final response = event.message;

      if (!mounted) return;
      setState(() {
        paths = response.paths;
        _isLoading = false;
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    return _isLoading ? Container() : RandomGrid(seed: 42, paths: paths);
  }
}
