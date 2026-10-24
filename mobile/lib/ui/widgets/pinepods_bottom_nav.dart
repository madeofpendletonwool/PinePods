// lib/ui/widgets/pinepods_bottom_nav.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/settings/bottom_bar_order.dart';

/// How many tabs stay in the bar when the rest move into the More sheet.
const int bottomBarPinnedCount = 4;

const double _minSlotWidth = 72.0;
const double _maxSlotWidth = 96.0;

IconData bottomNavIcon(String label, {bool selected = false}) {
  switch (label) {
    case 'Home':
      return selected ? PhosphorIconsFill.house : PhosphorIconsRegular.house;
    case 'Feed':
      return selected ? PhosphorIconsFill.rss : PhosphorIconsRegular.rss;
    case 'Saved':
      return selected
          ? PhosphorIconsFill.bookmarkSimple
          : PhosphorIconsRegular.bookmarkSimple;
    case 'Podcasts':
      return selected
          ? PhosphorIconsFill.microphoneStage
          : PhosphorIconsRegular.microphoneStage;
    case 'People':
      return selected ? PhosphorIconsFill.users : PhosphorIconsRegular.users;
    case 'Downloads':
      return selected
          ? PhosphorIconsFill.downloadSimple
          : PhosphorIconsRegular.downloadSimple;
    case 'History':
      return selected
          ? PhosphorIconsFill.clockCounterClockwise
          : PhosphorIconsRegular.clockCounterClockwise;
    case 'Playlists':
      return selected
          ? PhosphorIconsFill.playlist
          : PhosphorIconsRegular.playlist;
    case 'Search':
      return selected
          ? PhosphorIconsFill.magnifyingGlass
          : PhosphorIconsRegular.magnifyingGlass;
    default:
      return selected ? PhosphorIconsFill.house : PhosphorIconsRegular.house;
  }
}

/// Fixed (non-scrolling) bottom bar. Shows every page when they fit, otherwise
/// the first [bottomBarPinnedCount] plus a More tab that opens the rest.
/// Page indexes are positions in [order].
class PinepodsBottomNav extends StatelessWidget {
  final List<String> order;
  final int selectedIndex;
  final ValueChanged<int> onSelect;

  const PinepodsBottomNav({
    super.key,
    required this.order,
    required this.selectedIndex,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final bottomInset = MediaQuery.of(context).padding.bottom;
    final width = MediaQuery.of(context).size.width;

    final showAll = order.length * _minSlotWidth <= width;
    final pinned = showAll ? order : order.take(bottomBarPinnedCount).toList();
    final overflow =
        showAll ? const <String>[] : order.skip(bottomBarPinnedCount).toList();
    final moreSelected = !showAll && selectedIndex >= bottomBarPinnedCount;
    final slotCount = pinned.length + (overflow.isEmpty ? 0 : 1);

    final slots = <Widget>[
      for (var i = 0; i < pinned.length; i++)
        _NavSlot(
          icon: bottomNavIcon(pinned[i], selected: i == selectedIndex),
          label: pinned[i],
          selected: i == selectedIndex,
          onTap: () => onSelect(i),
        ),
      if (overflow.isNotEmpty)
        _NavSlot(
          icon: moreSelected
              ? bottomNavIcon(order[selectedIndex], selected: true)
              : PhosphorIconsRegular.dotsThree,
          label: moreSelected ? order[selectedIndex] : 'More',
          selected: moreSelected,
          onTap: () => _showMoreSheet(context, overflow),
        ),
    ];

    return Container(
      height: 70 + bottomInset,
      padding: EdgeInsets.only(bottom: bottomInset),
      decoration: BoxDecoration(
        color: theme.bottomAppBarTheme.color,
        border: Border(
          top: BorderSide(color: theme.dividerColor, width: 0.5),
        ),
      ),
      child: Center(
        child: ConstrainedBox(
          constraints: BoxConstraints(maxWidth: slotCount * _maxSlotWidth),
          child: Row(
            children: [for (final s in slots) Expanded(child: s)],
          ),
        ),
      ),
    );
  }

  void _showMoreSheet(BuildContext context, List<String> overflow) {
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) {
        final theme = Theme.of(sheetContext);
        return SafeArea(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                GridView.count(
                  crossAxisCount: 3,
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  mainAxisSpacing: 8,
                  crossAxisSpacing: 8,
                  childAspectRatio: 1.25,
                  children: [
                    for (var j = 0; j < overflow.length; j++)
                      _MoreTile(
                        label: overflow[j],
                        selected:
                            selectedIndex == bottomBarPinnedCount + j,
                        onTap: () {
                          Navigator.pop(sheetContext);
                          onSelect(bottomBarPinnedCount + j);
                        },
                      ),
                  ],
                ),
                const SizedBox(height: 8),
                TextButton.icon(
                  icon: const Icon(PhosphorIconsRegular.dotsSix, size: 18),
                  label: const Text('Reorder tabs'),
                  style: TextButton.styleFrom(
                    foregroundColor: theme.textTheme.bodySmall?.color,
                  ),
                  onPressed: () {
                    Navigator.pop(sheetContext);
                    Navigator.push(
                      context,
                      MaterialPageRoute<void>(
                        builder: (_) => const BottomBarOrderWidget(),
                      ),
                    );
                  },
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _NavSlot extends StatelessWidget {
  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _NavSlot({
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colour = selected
        ? theme.iconTheme.color
        : HSLColor.fromColor(theme.bottomAppBarTheme.color!)
            .withLightness(0.8)
            .toColor();

    // Opaque so the whole slot is tappable, not just the icon and text.
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap,
      child: Semantics(
        button: true,
        selected: selected,
        label: label,
        excludeSemantics: true,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(icon, color: colour, size: 24),
            const SizedBox(height: 4),
            Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: 11,
                color: colour,
                fontWeight: selected ? FontWeight.w600 : FontWeight.normal,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _MoreTile extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _MoreTile({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final fg = selected ? scheme.onPrimaryContainer : scheme.onSurface;

    return Material(
      color: selected
          ? scheme.primaryContainer
          : scheme.surfaceContainerHighest,
      borderRadius: BorderRadius.circular(16),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(bottomNavIcon(label, selected: selected), color: fg, size: 26),
            const SizedBox(height: 6),
            Text(
              label,
              style: TextStyle(
                fontSize: 13,
                color: fg,
                fontWeight: selected ? FontWeight.w600 : FontWeight.normal,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
