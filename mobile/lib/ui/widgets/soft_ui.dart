// lib/ui/widgets/soft_ui.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

/// Neutral foreground for secondary controls. Taken from the body text colour
/// rather than colorScheme.onSurface, which some themes set to their accent.
Color softNeutral(ThemeData theme) =>
    theme.textTheme.bodyLarge?.color ?? (theme.brightness == Brightness.dark ? Colors.white : Colors.black);

/// Subtle fill behind pills, circle buttons and search fields.
Color softFill(ThemeData theme) => softNeutral(theme).withOpacity(0.07);

Color onColor(Color color) =>
    ThemeData.estimateBrightnessForColor(color) == Brightness.dark ? Colors.white : Colors.black;

/// Single-select filter pill; filled with [accent] when selected.
class FilterPill extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;
  final IconData? icon;
  final Color? accent;

  const FilterPill({
    super.key,
    required this.label,
    required this.selected,
    required this.onTap,
    this.icon,
    this.accent,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = this.accent ?? theme.primaryColor;
    final fg = selected ? onColor(accent) : softNeutral(theme).withOpacity(0.8);
    return Material(
      color: selected ? accent : softFill(theme),
      shape: const StadiumBorder(),
      child: InkWell(
        customBorder: const StadiumBorder(),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[
                Icon(icon, size: 16, color: fg),
                const SizedBox(width: 6),
              ],
              Text(label, style: TextStyle(fontSize: 13.5, fontWeight: FontWeight.w600, color: fg)),
            ],
          ),
        ),
      ),
    );
  }
}

/// Round secondary button, e.g. beside a primary pill.
class CircleAction extends StatelessWidget {
  final String? tooltip;
  final Widget icon;
  final VoidCallback? onTap;
  final double size;

  const CircleAction({super.key, required this.icon, this.tooltip, this.onTap, this.size = 46});

  @override
  Widget build(BuildContext context) {
    final button = Material(
      color: softFill(Theme.of(context)),
      shape: const CircleBorder(),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(width: size, height: size, child: Center(child: icon)),
      ),
    );
    return tooltip == null ? button : Tooltip(message: tooltip!, child: button);
  }
}

/// Fixed-height pinned sliver header.
class PinnedBarDelegate extends SliverPersistentHeaderDelegate {
  final double height;
  final Color color;
  final Widget child;

  PinnedBarDelegate({required this.height, required this.color, required this.child});

  @override
  double get minExtent => height;

  @override
  double get maxExtent => height;

  @override
  Widget build(BuildContext context, double shrinkOffset, bool overlapsContent) {
    return Material(
      color: color,
      elevation: overlapsContent || shrinkOffset > 0 ? 1 : 0,
      child: child,
    );
  }

  @override
  bool shouldRebuild(PinnedBarDelegate old) => true;
}

class ToolbarFilter {
  final String label;
  final bool selected;
  final VoidCallback onTap;
  final IconData? icon;

  const ToolbarFilter({required this.label, required this.selected, required this.onTap, this.icon});
}

/// Pinned list toolbar: filter pills, a search field that opens in place, and
/// a sort menu. Sits in a [SliverPersistentHeader] via [ListToolbar.sliver].
class ListToolbar<T> extends StatefulWidget {
  static const double height = 56;

  final List<ToolbarFilter> filters;
  final TextEditingController? searchController;
  final String searchHint;
  final Map<T, String>? sortOptions;
  final T? sortValue;
  final ValueChanged<T>? onSort;
  final List<Widget> trailing;
  final Color? accent;

  const ListToolbar({
    super.key,
    this.filters = const [],
    this.searchController,
    this.searchHint = 'Search',
    this.sortOptions,
    this.sortValue,
    this.onSort,
    this.trailing = const [],
    this.accent,
  });

  /// The toolbar pinned under whatever app bar is above it.
  static Widget sliver<T>(BuildContext context, ListToolbar<T> toolbar) {
    return SliverPersistentHeader(
      pinned: true,
      delegate: PinnedBarDelegate(
        height: height,
        color: Theme.of(context).scaffoldBackgroundColor,
        child: toolbar,
      ),
    );
  }

  @override
  State<ListToolbar<T>> createState() => _ListToolbarState<T>();
}

class _ListToolbarState<T> extends State<ListToolbar<T>> {
  // Opens already expanded when there's an active query (e.g. after a rebuild).
  late bool _searchOpen = widget.searchController?.text.isNotEmpty ?? false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final controller = widget.searchController;

    if (_searchOpen && controller != null) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(16, 6, 8, 6),
        child: Row(
          children: [
            Expanded(
              child: TextField(
                controller: controller,
                autofocus: true,
                textInputAction: TextInputAction.search,
                decoration: InputDecoration(
                  hintText: widget.searchHint,
                  isDense: true,
                  prefixIcon: const Icon(PhosphorIconsRegular.magnifyingGlass, size: 20),
                  filled: true,
                  fillColor: softFill(theme),
                  border: OutlineInputBorder(borderRadius: BorderRadius.circular(22), borderSide: BorderSide.none),
                  enabledBorder:
                      OutlineInputBorder(borderRadius: BorderRadius.circular(22), borderSide: BorderSide.none),
                  focusedBorder:
                      OutlineInputBorder(borderRadius: BorderRadius.circular(22), borderSide: BorderSide.none),
                  contentPadding: const EdgeInsets.symmetric(vertical: 10),
                ),
              ),
            ),
            IconButton(
              tooltip: 'Close search',
              icon: const Icon(PhosphorIconsRegular.x),
              onPressed: () {
                controller.clear();
                setState(() => _searchOpen = false);
              },
            ),
          ],
        ),
      );
    }

    final sortOptions = widget.sortOptions;
    return Row(
      children: [
        Expanded(
          child: ListView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
            children: [
              for (final f in widget.filters)
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: FilterPill(
                    label: f.label,
                    icon: f.icon,
                    selected: f.selected,
                    accent: widget.accent,
                    onTap: f.onTap,
                  ),
                ),
            ],
          ),
        ),
        if (controller != null)
          IconButton(
            tooltip: 'Search',
            icon: const Icon(PhosphorIconsRegular.magnifyingGlass),
            onPressed: () => setState(() => _searchOpen = true),
          ),
        if (sortOptions != null && widget.onSort != null)
          PopupMenuButton<T>(
            tooltip: 'Sort',
            icon: const Icon(PhosphorIconsRegular.sortAscending),
            onSelected: widget.onSort,
            itemBuilder: (context) => [
              for (final entry in sortOptions.entries)
                CheckedPopupMenuItem<T>(
                  value: entry.key,
                  checked: entry.key == widget.sortValue,
                  child: Text(entry.value),
                ),
            ],
          ),
        ...widget.trailing,
        const SizedBox(width: 4),
      ],
    );
  }
}

/// Quiet empty/error state: small icon, title, one line, optional action.
class SoftEmptyState extends StatelessWidget {
  final IconData icon;
  final String title;
  final String? body;
  final Widget? action;

  const SoftEmptyState({super.key, required this.icon, required this.title, this.body, this.action});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = theme.hintColor;
    return Padding(
      padding: const EdgeInsets.fromLTRB(32, 40, 32, 48),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 40, color: muted),
          const SizedBox(height: 12),
          Text(title, style: theme.textTheme.titleMedium, textAlign: TextAlign.center),
          if (body != null) ...[
            const SizedBox(height: 6),
            Text(body!, style: theme.textTheme.bodyMedium!.copyWith(color: muted), textAlign: TextAlign.center),
          ],
          if (action != null) ...[const SizedBox(height: 16), action!],
        ],
      ),
    );
  }
}

/// Slim section title with an optional count and trailing actions.
class SoftSectionHeader extends StatelessWidget {
  final String title;
  final int? count;
  final List<Widget> trailing;
  final EdgeInsetsGeometry padding;

  const SoftSectionHeader({
    super.key,
    required this.title,
    this.count,
    this.trailing = const [],
    this.padding = const EdgeInsets.fromLTRB(16, 12, 8, 0),
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: padding,
      child: Row(
        children: [
          Text(title, style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold)),
          if (count != null) ...[
            const SizedBox(width: 8),
            Text('$count', style: theme.textTheme.titleMedium!.copyWith(color: theme.hintColor)),
          ],
          const Spacer(),
          ...trailing,
        ],
      ),
    );
  }
}
