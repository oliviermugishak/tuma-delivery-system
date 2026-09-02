import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:google_maps_flutter/google_maps_flutter.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/geo_api.dart';
import 'package:tuma_app/core/api/models/address.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/shared/widgets/show_app_snack.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/location/delivery_pin_map.dart'
    show PasteCoordinatesField, kigaliCenter;
import 'package:tuma_app/features/tracking/delivery_map.dart'
    show isMobilePlatform;

/// The delivery-location editor — the redesign's first-class location:
/// a full-screen Google map with the pin FIXED AT CENTER. The customer
/// moves the map, never the pin; the reverse-geocoded street line fills
/// itself in, a search jumps anywhere, and GPS is one button away. What
/// saves is a real address row (pin + text + Home/Work/Other + rider
/// note) — checkout and the rider's card both read from it.
///
/// Without a geocoding key behind the server's proxy, the map still
/// works and the address line stays honestly empty for typing (P2).
class DeliveryLocationScreen extends ConsumerStatefulWidget {
  const DeliveryLocationScreen({super.key, this.edit});

  /// An address being edited — the card arrives pre-filled and Save
  /// patches it instead of creating a new row.
  final Address? edit;

  @override
  ConsumerState<DeliveryLocationScreen> createState() =>
      _DeliveryLocationScreenState();
}

class _DeliveryLocationScreenState
    extends ConsumerState<DeliveryLocationScreen> {
  static const _kinds = [
    (kind: 'home', label: 'Home'),
    (kind: 'work', label: 'Work'),
    (kind: 'other', label: 'Other'),
  ];

  GoogleMapController? _map;

  /// The camera's center — ON MOBILE THIS IS THE PIN. `onCameraMove`
  /// tracks it; nothing else may write it.
  LatLng _target = LatLng(kigaliCenter.lat, kigaliCenter.lng);
  CustomerLocation? _pendingSeed;
  CustomerLocation? _desktopPin;

  late final TextEditingController _addressCtrl;
  late final TextEditingController _noteCtrl;
  late final FocusNode _searchFocus;
  final _searchCtrl = TextEditingController();
  Timer? _searchDebounce;
  List<GeoHit> _hits = const [];

  String _kind = 'home';
  bool _reverseBusy = false;
  int _reverseSeq = 0;
  bool _suppressIdleOnce = false;
  bool _locating = false;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final edit = widget.edit;
    if (edit != null) {
      if (edit.lat != null && edit.lng != null) {
        _target = LatLng(edit.lat!, edit.lng!);
      }
      _addressCtrl = TextEditingController(text: edit.addressText);
      _noteCtrl = TextEditingController(text: edit.note ?? '');
      _kind = _kinds.any((k) => k.kind == edit.kind) ? edit.kind : 'other';
    } else {
      _addressCtrl = TextEditingController();
      _noteCtrl = TextEditingController();
      _initSeed();
    }
    _searchFocus = FocusNode();
  }

  @override
  void dispose() {
    _map?.dispose();
    _addressCtrl.dispose();
    _noteCtrl.dispose();
    _searchCtrl.dispose();
    _searchFocus.dispose();
    _searchDebounce?.cancel();
    super.dispose();
  }

  /// A fresh editor starts where the customer's persisted pin is —
  /// checkout's earlier choice, a GPS fix — or Kigali center.
  Future<void> _initSeed() async {
    final persisted = await ref.read(customerLocationProvider.future);
    if (!mounted || persisted == null) return;
    final controller = _map;
    if (controller == null) {
      _pendingSeed = persisted;
    } else {
      await controller.animateCamera(
        CameraUpdate.newLatLngZoom(LatLng(persisted.lat, persisted.lng), 16),
      );
    }
  }

  // --- camera-driven pin ---------------------------------------------------

  void _onCameraMove(CameraPosition position) {
    _target = position.target;
  }

  Future<void> _onCameraIdle() async {
    if (!isMobilePlatform) return;
    if (_suppressIdleOnce) {
      _suppressIdleOnce = false;
      return;
    }
    final target = _target;
    final seq = ++_reverseSeq;
    setState(() => _reverseBusy = true);
    try {
      final hits = await ref
          .read(geoApiProvider)
          .reverse(lat: target.latitude, lng: target.longitude);
      if (!mounted || seq != _reverseSeq) return;
      if (hits.isNotEmpty) _addressCtrl.text = hits.first.addressText;
    } on ApiError {
      // No geocoder behind the proxy: the field stays as-is for typing.
    } finally {
      if (mounted) setState(() => _reverseBusy = false);
    }
  }

  // --- search --------------------------------------------------------------

  void _onSearchChanged(String query) {
    _searchDebounce?.cancel();
    if (query.trim().length < 3) {
      if (_hits.isNotEmpty) setState(() => _hits = const []);
      return;
    }
    _searchDebounce = Timer(const Duration(milliseconds: 350), () async {
      try {
        final hits = await ref.read(geoApiProvider).search(query.trim());
        if (mounted) setState(() => _hits = hits.take(5).toList());
      } on ApiError {
        if (mounted) setState(() => _hits = const []);
      }
    });
  }

  Future<void> _goTo(GeoHit hit) async {
    _searchFocus.unfocus();
    _searchCtrl.clear();
    setState(() => _hits = const []);
    _addressCtrl.text = hit.addressText;
    // The text is already the truth for this point; let the idle handler
    // skip its re-geocode.
    _suppressIdleOnce = true;
    await _map?.animateCamera(
      CameraUpdate.newLatLngZoom(LatLng(hit.lat, hit.lng), 17),
    );
  }

  // --- GPS -----------------------------------------------------------------

  Future<void> _useMyLocation() async {
    if (_locating) return;
    setState(() => _locating = true);
    try {
      final fix = await ref.read(acquireLocationProvider)();
      // The acquire can take seconds; this State (and the controller
      // with it) may be gone by the time the fix lands (A40).
      if (!mounted) return;
      await _map?.animateCamera(
        CameraUpdate.newLatLngZoom(LatLng(fix.lat, fix.lng), 17),
      );
    } on Object {
      if (mounted) {
        showAppSnack(
          context,
          'Could not get your location — move the map instead.',
          duration: const Duration(seconds: 2),
        );
      }
    } finally {
      if (mounted) setState(() => _locating = false);
    }
  }

  // --- save ----------------------------------------------------------------

  Future<void> _save() async {
    final CustomerLocation? pin;
    if (isMobilePlatform) {
      pin = CustomerLocation(lat: _target.latitude, lng: _target.longitude);
    } else {
      pin = _desktopPin;
    }
    final addressText = _addressCtrl.text.trim();
    if (pin == null) {
      setState(() => _error = 'Drop the pin first — paste coordinates above.');
      return;
    }
    if (addressText.isEmpty) {
      setState(
        () => _error = 'Name the spot — the rider needs a line to read.',
      );
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final label = _kinds.firstWhere((k) => k.kind == _kind).label;
      final address = Address(
        id: widget.edit?.id ?? '',
        label: label,
        addressText: addressText,
        lat: pin.lat,
        lng: pin.lng,
        isDefault: widget.edit?.isDefault ?? false,
        kind: _kind,
        note: _noteCtrl.text.trim().isEmpty ? null : _noteCtrl.text.trim(),
      );
      final saved = widget.edit == null
          ? await ref.read(addressApiProvider).create(address)
          : await ref.read(addressApiProvider).update(address);
      // Home's distances and any later map seed sharpen from this pin.
      await ref.read(customerLocationProvider.notifier).setPin(pin);
      if (!mounted) return;
      showAppSnack(
        context,
        widget.edit == null
            ? '${saved.label} saved — ${saved.addressText}'
            : 'Location updated.',
        duration: const Duration(seconds: 2),
      );
      // A cold start on /profile/location has nothing to pop — the saved
      // pin still deserves a landing, not a throw (review P16).
      if (context.canPop()) {
        context.pop();
      } else {
        context.go('/home');
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = e.message;
        });
      }
    }
  }

  // --- build ---------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final mobile = isMobilePlatform;

    return Scaffold(
      backgroundColor: AppColors.surface,
      resizeToAvoidBottomInset: false,
      body: Stack(
        children: [
          if (mobile)
            GoogleMap(
              initialCameraPosition: CameraPosition(target: _target, zoom: 16),
              onMapCreated: (controller) {
                _map = controller;
                final seed = _pendingSeed;
                if (seed != null) {
                  _pendingSeed = null;
                  controller.animateCamera(
                    CameraUpdate.newLatLngZoom(LatLng(seed.lat, seed.lng), 16),
                  );
                }
              },
              onCameraMove: _onCameraMove,
              onCameraIdle: _onCameraIdle,
              myLocationEnabled: true,
              myLocationButtonEnabled: false,
              zoomControlsEnabled: false,
              mapToolbarEnabled: false,
              compassEnabled: false,
            )
          else
            SafeArea(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 80, 20, 0),
                child: PasteCoordinatesField(
                  pin: _desktopPin,
                  onPin: (pin) => setState(() {
                    _desktopPin = pin;
                    _addressCtrl.text =
                        '${pin.lat.toStringAsFixed(6)}, ${pin.lng.toStringAsFixed(6)}';
                  }),
                ),
              ),
            ),

          // THE PIN — fixed at center; the map moves under it. The tip
          // lands exactly on the camera target.
          if (mobile)
            IgnorePointer(
              child: Center(
                child: Transform.translate(
                  offset: const Offset(0, -21),
                  child: const Icon(
                    Icons.location_on_rounded,
                    size: 42,
                    color: AppColors.primary,
                    shadows: [Shadow(color: AppColors.surface, blurRadius: 6)],
                  ),
                ),
              ),
            ),

          // SEARCH — over the map, under the status bar.
          if (mobile)
            SafeArea(
              child: Column(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(12, 8, 16, 0),
                    child: _SearchBar(
                      controller: _searchCtrl,
                      focusNode: _searchFocus,
                      onChanged: _onSearchChanged,
                      // A cold start on /profile/location has nothing to
                      // pop — fall home instead of throwing (review P16).
                      onBack: () {
                        if (context.canPop()) {
                          context.pop();
                        } else {
                          context.go('/home');
                        }
                      },
                    ),
                  ),
                  if (_hits.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                      child: _SearchResults(hits: _hits, onTap: _goTo),
                    ),
                ],
              ),
            ),

          // BOTTOM SECTION — my-location FAB + the "map moves" pill + the
          // save card. The card rises above the keyboard (viewInsets) so
          // Save stays reachable while typing the address or note — the
          // map stays full-bleed behind it (resizeToAvoidBottomInset is
          // false on purpose).
          Positioned(
            left: 0,
            right: 0,
            bottom: MediaQuery.viewInsetsOf(context).bottom,
            child: Column(
              children: [
                Stack(
                  children: [
                    Align(
                      child: mobile
                          ? const _MoveTheMapPill()
                          : const SizedBox.shrink(),
                    ),
                    Align(
                      alignment: Alignment.centerRight,
                      child: Padding(
                        padding: const EdgeInsets.only(right: 16),
                        child: mobile
                            ? _LocateFab(
                                locating: _locating,
                                onPressed: _useMyLocation,
                              )
                            : const SizedBox.shrink(),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                _SaveCard(
                  addressCtrl: _addressCtrl,
                  noteCtrl: _noteCtrl,
                  kind: _kind,
                  onKind: (kind) => setState(() => _kind = kind),
                  reverseBusy: _reverseBusy,
                  saving: _saving,
                  error: _error,
                  onSave: _save,
                  isEdit: widget.edit != null,
                  textTheme: textTheme,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------------

class _SearchBar extends StatelessWidget {
  const _SearchBar({
    required this.controller,
    required this.focusNode,
    required this.onChanged,
    required this.onBack,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final ValueChanged<String> onChanged;
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 46,
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        children: [
          IconButton(
            icon: const Icon(
              Icons.arrow_back_ios_new_rounded,
              size: 18,
              color: AppColors.onSurface,
            ),
            onPressed: onBack,
          ),
          Expanded(
            child: TextField(
              controller: controller,
              focusNode: focusNode,
              onChanged: onChanged,
              style: AppTheme.bd(Theme.of(context).textTheme),
              decoration: const InputDecoration(
                hintText: 'Search a street or place…',
                border: InputBorder.none,
                isDense: true,
              ),
            ),
          ),
          const Padding(
            padding: EdgeInsets.only(right: 14),
            child: Icon(
              Icons.search_rounded,
              size: 20,
              color: AppColors.onSurfaceMuted,
            ),
          ),
        ],
      ),
    );
  }
}

class _SearchResults extends StatelessWidget {
  const _SearchResults({required this.hits, required this.onTap});

  final List<GeoHit> hits;
  final ValueChanged<GeoHit> onTap;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child: Material(
          color: AppColors.surfaceAlt,
          child: Column(
            children: [
              for (final (index, hit) in hits.indexed) ...[
                if (index > 0)
                  const Divider(height: 1, color: AppColors.surfaceBorder),
                InkWell(
                  onTap: () => onTap(hit),
                  borderRadius: index == 0
                      ? const BorderRadius.vertical(top: Radius.circular(16))
                      : index == hits.length - 1
                      ? const BorderRadius.vertical(bottom: Radius.circular(16))
                      : null,
                  child: SizedBox(
                    width: double.infinity,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 12,
                      ),
                      child: Text(
                        hit.addressText,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AppTheme.bd(Theme.of(context).textTheme),
                      ),
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _MoveTheMapPill extends StatelessWidget {
  const _MoveTheMapPill();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(
            Icons.open_with_rounded,
            size: 14,
            color: AppColors.primary,
          ),
          const SizedBox(width: 6),
          Text(
            'Move the map — the pin stays',
            style: AppTheme.cap(Theme.of(context).textTheme)
                .copyWith(fontWeight: FontWeight.w700),
          ),
        ],
      ),
    );
  }
}

class _LocateFab extends StatelessWidget {
  const _LocateFab({required this.locating, required this.onPressed});

  final bool locating;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surfaceAlt,
      shape: const CircleBorder(
        side: BorderSide(color: AppColors.surfaceBorder),
      ),
      child: InkWell(
        onTap: onPressed,
        customBorder: const CircleBorder(),
        child: SizedBox(
          width: 46,
          height: 46,
          child: Center(
            child: locating
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(
                    Icons.my_location_rounded,
                    size: 20,
                    color: AppColors.primary,
                  ),
          ),
        ),
      ),
    );
  }
}

class _SaveCard extends StatelessWidget {
  const _SaveCard({
    required this.addressCtrl,
    required this.noteCtrl,
    required this.kind,
    required this.onKind,
    required this.reverseBusy,
    required this.saving,
    required this.error,
    required this.onSave,
    required this.isEdit,
    required this.textTheme,
  });

  final TextEditingController addressCtrl;
  final TextEditingController noteCtrl;
  final String kind;
  final ValueChanged<String> onKind;
  final bool reverseBusy;
  final bool saving;
  final String? error;
  final VoidCallback onSave;
  final bool isEdit;
  final TextTheme textTheme;

  static const _kindLabels = {'home': 'Home', 'work': 'Work', 'other': 'Other'};

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(16, 10, 16, 0),
      decoration: const BoxDecoration(
        color: AppColors.surface,
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
        border: Border(top: BorderSide(color: AppColors.surfaceBorder)),
      ),
      child: SafeArea(
        top: false,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Center(
              child: Container(
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: AppColors.onSurfaceMuted.withValues(alpha: 0.4),
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: addressCtrl,
                    style: AppTheme.hd(textTheme)
                        .copyWith(fontWeight: FontWeight.w700),
                    maxLines: 1,
                    decoration: InputDecoration(
                      hintText: 'Street, building, landmark…',
                      isDense: true,
                      border: InputBorder.none,
                      suffixIcon: reverseBusy
                          ? const Padding(
                              padding: EdgeInsets.all(12),
                              child: SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              ),
                            )
                          : null,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            // Wrap, not Row: three intrinsic-width chips overflow a
            // 320dp phone by a few px and the stripe painted over the
            // "Other" button (the founder's small-phone overflow).
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final entry in _kindLabels.entries)
                  _KindChip(
                    label: entry.value,
                    selected: kind == entry.key,
                    onTap: () => onKind(entry.key),
                  ),
              ],
            ),
            const SizedBox(height: 12),
            TextField(
              controller: noteCtrl,
              maxLength: 140,
              style: AppTheme.bd(textTheme),
              decoration: const InputDecoration(
                hintText: 'Near Kicukiro · Gate on the left side…',
                counterText: '',
                isDense: true,
              ),
            ),
            if (error != null) ...[
              const SizedBox(height: 8),
              Text(
                error!,
                style: AppTheme.sub(textTheme).copyWith(color: AppColors.error),
              ),
            ],
            const SizedBox(height: 14),
            FilledButton(
              onPressed: saving ? null : onSave,
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
                disabledBackgroundColor: AppColors.onSurface.withValues(
                  alpha: 0.15,
                ),
              ),
              child: Text(
                saving
                    ? 'Saving…'
                    : isEdit
                    ? 'Update location'
                    : 'Save location',
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }
}

class _KindChip extends StatelessWidget {
  const _KindChip({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(999),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        decoration: BoxDecoration(
          color: selected ? AppColors.primary : Colors.transparent,
          borderRadius: BorderRadius.circular(999),
          border: Border.all(
            color: selected ? AppColors.primary : AppColors.surfaceBorder,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              switch (label) {
                'Home' => Icons.home_rounded,
                'Work' => Icons.work_rounded,
                _ => Icons.place_rounded,
              },
              size: 15,
              color: selected ? AppColors.onSurface : AppColors.onSurfaceMuted,
            ),
            const SizedBox(width: 6),
            Text(
              label,
              style: AppTheme.bd(Theme.of(context).textTheme).copyWith(
                fontWeight: FontWeight.w600,
                color: selected
                    ? AppColors.onSurface
                    : AppColors.onSurfaceMuted,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
