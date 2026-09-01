import 'package:flutter/material.dart';

/// A lazily-built row grid as a sliver: chunks [itemCount] cells into
/// rows of [columns], one [SliverList] item per row, so only visible
/// rows build. It replaces the eager `Wrap`-inside-a-ListView pattern —
/// which built every cell (and resolved every image provider) up front —
/// while keeping the same layout contract: [cellWidth]-wide cells,
/// left-aligned rows, [spacing] between rows and columns, and cells keep
/// their intrinsic heights (no forced uniform tile height).
class SliverRowGrid extends StatelessWidget {
  const SliverRowGrid({
    super.key,
    required this.itemCount,
    required this.columns,
    required this.cellWidth,
    required this.spacing,
    required this.itemBuilder,
  });

  final int itemCount;
  final int columns;
  final double cellWidth;
  final double spacing;
  final Widget Function(BuildContext context, int index) itemBuilder;

  @override
  Widget build(BuildContext context) {
    final rowCount = (itemCount + columns - 1) ~/ columns;
    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, row) {
          final start = row * columns;
          final end = (start + columns).clamp(0, itemCount);
          return Padding(
            padding: EdgeInsets.only(bottom: row == rowCount - 1 ? 0 : spacing),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (var i = start; i < end; i++) ...[
                  // The gap lives BETWEEN the cells — the row reserves
                  // (columns-1) gaps in cellWidth's math, but a Row stacks
                  // children edge-to-edge, so without this the cards
                  // touch and the reserved space dangles at row's end.
                  if (i > start) SizedBox(width: spacing),
                  SizedBox(
                    width: cellWidth,
                    child: itemBuilder(context, i),
                  ),
                ],
              ],
            ),
          );
        },
        childCount: rowCount,
      ),
    );
  }
}
