import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:google_maps_flutter/google_maps_flutter.dart';

import 'package:tuma_app/core/api/api_client.dart';
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

/// The delivery-location surface — two modes:
///
/// **List mode** (no [edit] address): "Your saved locations" fetched from
/// the backend. Each card shows kind icon, label, street line, and the
/// rider note under it. Tap to edit (self-pushes this route with the
/// address as extra), trash to delete (confirmed). An add button opens a
/// blank editor. The map sits behind on mobile; desktop keeps its paste
/// panel for coordinates.
///
/// **Editor mode** ([edit] passed, or "add new" tapped): the full-screen
/// map with the pin FIXED AT CENTER on mobile — the customer moves the
/// map, never the pin; GPS is one button away. What saves is a real
/// address row (pin + text + Home/Work/Other + rider note) — checkout
/// and the rider's card both read from it. The pin is the truth; the
/// text is a short human label ("Home", "near Simba") typed by the
/// customer — nothing geocodes it.
class DeliveryLocationScreen extends ConsumerStatefulWidget {
  const DeliveryLocationScreen({super.key, this.edit, this.fresh = false});

  /// An address being edited — the card arrives pre-filled and Save
  /// patches it instead of creating a new row. Null alone = list mode.
  final Address? edit;

  /// The list's "Add new location" door: opens the BLANK editor. (An
  /// `extra` can't carry this — the address-less push must stay
  /// distinguishable from the list's own route.)
  final bool fresh;

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

  // --- shared state --------------------------------------------------------

  bool _loadingList = false;
  List<Address>? _savedAddresses;
  String? _listError;

  // --- editor state --------------------------------------------------------

  GoogleMapController? _map;
  LatLng _target = LatLng(kigaliCenter.lat, kigaliCenter.lng);
  CustomerLocation? _desktopPin;

  late final TextEditingController _addressCtrl;
  late final TextEditingController _noteCtrl;

  String _kind = 'home';
  bool _locating = false;
  bool _saving = false;
  String? _editorError;

  /// List mode is the default fresh entry; an address or ?mode=new means
  /// the editor instead.
  bool get _isEditing => widget.edit != null || widget.fresh;

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
      if (!widget.fresh) unawaited(_loadAddresses());
    }
  }

  @override
  void dispose() {
    _map?.dispose();
    _addressCtrl.dispose();
    _noteCtrl.dispose();
    super.dispose();
  }

  // --- list mode -----------------------------------------------------------

  Future<void> _loadAddresses() async {
    setState(() {
      _loadingList = true;
      _listError = null;
    });
    try {
      final list = await ref.read(addressApiProvider).list();
      if (!mounted) return;
      setState(() {
        _savedAddresses = list;
        _loadingList = false;
      });
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() {
        _listError = e.message;
        _loadingList = false;
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _listError = 'Could not load your saved locations.';
        _loadingList = false;
      });
    }
  }

  Future<void> _deleteAddress(Address address) async {
    final confirmed = await showDialog<bool>(
      context: context,
      // Outside-tap dismisses (= keep); the buttons are the explicit paths.
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('Delete this location?'),
        content: Text(
          '${address.label} · ${address.addressText}\n\nThe rider will no longer see this address.',
          style: AppTheme.bd(Theme.of(ctx).textTheme),
        ),
        actions: [
          // Side by side — Cancel beside Delete, never stacked above it.
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: FilledButton.styleFrom(
              backgroundColor: AppColors.error,
              minimumSize: const Size(0, 44),
              padding: const EdgeInsets.symmetric(horizontal: 24),
            ),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    try {
      await ref.read(addressApiProvider).delete(address.id);
      if (!mounted) return;
      showAppSnack(context, '${address.label} deleted.');
      await _loadAddresses();
    } on ApiError catch (e) {
      if (!mounted) return;
      showAppSnack(context, e.message);
    } on Object {
      if (!mounted) return;
      showAppSnack(context, 'Could not delete the location.');
    }
  }

  Future<void> _openEditor([Address? address]) async {
    // A card passes its address (edit); the add button passes nothing and
    // flags the BLANK editor via ?mode=new — a plain extra-less push
    // would re-open the list itself.
    await context.push(
      address == null ? '/profile/location?mode=new' : '/profile/location',
      extra: address,
    );
    // The editor changed the book (save, delete) — the list must show it.
    if (mounted) await _loadAddresses();
  }

  // --- editor: camera-driven pin ------------------------------------------

  void _onCameraMove(CameraPosition position) {
    _target = position.target;
  }

  // --- editor: GPS --------------------------------------------------------

  Future<void> _useMyLocation() async {
    if (_locating) return;
    setState(() => _locating = true);
    try {
      final fix = await ref.read(acquireLocationProvider)();
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

  // --- editor: save -------------------------------------------------------

  Future<void> _save() async {
    final CustomerLocation? pin;
    if (isMobilePlatform) {
      pin = CustomerLocation(lat: _target.latitude, lng: _target.longitude);
    } else {
      pin = _desktopPin;
    }
    final addressText = _addressCtrl.text.trim();
    if (pin == null) {
      setState(() => _editorError = 'Drop the pin first — paste coordinates above.');
      return;
    }
    if (addressText.isEmpty) {
      setState(
        () => _editorError = 'Name the spot — the rider needs a line to read.',
      );
      return;
    }
    setState(() {
      _saving = true;
      _editorError = null;
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
      if (!mounted) return;
      showAppSnack(
        context,
        widget.edit == null
            ? '${saved.label} saved — ${saved.addressText}'
            : 'Location updated.',
        duration: const Duration(seconds: 2),
      );
      if (context.canPop()) {
        context.pop();
      } else {
        context.go('/home');
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() {
          _saving = false;
          _editorError = e.message;
        });
      }
    }
  }

  // --- editor: desktop paste ----------------------------------------------

  Future<void> _onDesktopPin(CustomerLocation pin) async {
    setState(() {
      _desktopPin = pin;
      _addressCtrl.text =
          '${pin.lat.toStringAsFixed(6)}, ${pin.lng.toStringAsFixed(6)}';
    });
  }

  // --- build --------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    if (_isEditing) return _buildEditor();
    return _buildList();
  }

  Widget _buildList() {
    final textTheme = Theme.of(context).textTheme;
    final mobile = isMobilePlatform;
    final addresses = _savedAddresses;

    return Scaffold(
      backgroundColor: AppColors.canvas,
      body: Stack(
        children: [
          // Map behind on mobile (visual context); plain canvas on desktop.
          if (mobile)
            GoogleMap(
              initialCameraPosition:
                  CameraPosition(target: kigaliCenter.toLatLng(), zoom: 12),
              myLocationEnabled: false,
              myLocationButtonEnabled: false,
              zoomControlsEnabled: false,
              mapToolbarEnabled: false,
              compassEnabled: false,
              liteModeEnabled: true,
            ),

          SafeArea(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 8, 16, 0),
                  child: Row(
                    children: [
                      _BackFab(onPressed: () {
                        if (context.canPop()) {
                          context.pop();
                        } else {
                          context.go('/home');
                        }
                      }),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          'Your saved locations',
                          style: AppTheme.d1(textTheme).copyWith(fontSize: 21),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 12),
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: _loadingList
                        ? const Center(child: CircularProgressIndicator())
                        : _listError != null
                            ? _ListError(
                                message: _listError!,
                                onRetry: _loadAddresses,
                                textTheme: textTheme,
                              )
                            : addresses == null || addresses.isEmpty
                                ? _EmptyList(onAdd: () => _openEditor())
                                : ListView.separated(
                                    itemCount: addresses.length,
                                    separatorBuilder: (_, _) =>
                                        const SizedBox(height: 10),
                                    itemBuilder: (_, i) => _AddressCard(
                                      address: addresses[i],
                                      onTap: () => _openEditor(addresses[i]),
                                      onDelete: () =>
                                          _deleteAddress(addresses[i]),
                                      textTheme: textTheme,
                                    ),
                                  ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 10, 16, 16),
                  child: FilledButton.icon(
                    onPressed: () => _openEditor(),
                    icon: const Icon(Icons.add_rounded, size: 20),
                    label: const Text(
                      'Add new location',
                      style: TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    style: FilledButton.styleFrom(
                      minimumSize: const Size.fromHeight(52),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildEditor() {
    final textTheme = Theme.of(context).textTheme;
    final mobile = isMobilePlatform;

    return Scaffold(
      backgroundColor: AppColors.canvas,
      resizeToAvoidBottomInset: false,
      body: Stack(
        children: [
          if (mobile)
            GoogleMap(
              initialCameraPosition: CameraPosition(target: _target, zoom: 16),
              onMapCreated: (controller) => _map = controller,
              onCameraMove: _onCameraMove,
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
                  onPin: _onDesktopPin,
                ),
              ),
            ),

          // THE PIN — fixed at center; the map moves under it.
          if (mobile)
            IgnorePointer(
              child: Center(
                child: Transform.translate(
                  offset: const Offset(0, -21),
                  child: const Icon(
                    Icons.location_on_rounded,
                    size: 42,
                    color: AppColors.primary,
                    shadows: [Shadow(color: Color(0x73141512), blurRadius: 6)],
                  ),
                ),
              ),
            ),

          // BACK — over the map on mobile, over the paste panel on desktop.
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 0, 0),
              child: _BackFab(onPressed: () {
                if (context.canPop()) {
                  context.pop();
                } else {
                  context.go('/home');
                }
              }),
            ),
          ),

          // BOTTOM SECTION — locate FAB + "map moves" pill + save card.
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
                  saving: _saving,
                  error: _editorError,
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
// List-mode widgets
// ---------------------------------------------------------------------------

class _AddressCard extends StatelessWidget {
  const _AddressCard({
    required this.address,
    required this.onTap,
    required this.onDelete,
    required this.textTheme,
  });

  final Address address;
  final VoidCallback onTap;
  final VoidCallback onDelete;
  final TextTheme textTheme;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface,
      borderRadius: BorderRadius.circular(16),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(16),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(14, 12, 6, 12),
          child: Row(
            children: [
              Icon(
                switch (address.kind) {
                  'home' => Icons.home_rounded,
                  'work' => Icons.work_rounded,
                  _ => Icons.place_rounded,
                },
                size: 22,
                color: AppColors.primary,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      address.label,
                      style: AppTheme.bd(textTheme)
                          .copyWith(fontWeight: FontWeight.w600),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      address.addressText,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: AppTheme.sub(textTheme),
                    ),
                    if (address.note != null && address.note!.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Text(
                        address.note!,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: AppTheme.sub(textTheme)
                            .copyWith(color: AppColors.onSurfaceMuted),
                      ),
                    ],
                  ],
                ),
              ),
              IconButton(
                onPressed: onDelete,
                icon: const Icon(Icons.delete_outline_rounded, size: 20),
                color: AppColors.onSurfaceMuted,
                tooltip: 'Delete',
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _EmptyList extends StatelessWidget {
  const _EmptyList({required this.onAdd});
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.place_outlined, size: 48, color: AppColors.onSurfaceMuted),
          const SizedBox(height: 12),
          Text(
            'No saved locations yet',
            style: AppTheme.bd(textTheme).copyWith(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 4),
          Text(
            'Add your home, work, or any place you order to.',
            style: AppTheme.sub(textTheme).copyWith(color: AppColors.onSurfaceMuted),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 20),
          FilledButton.icon(
            onPressed: onAdd,
            icon: const Icon(Icons.add_rounded, size: 18),
            label: const Text('Add your first location'),
          ),
        ],
      ),
    );
  }
}

class _ListError extends StatelessWidget {
  const _ListError({
    required this.message,
    required this.onRetry,
    required this.textTheme,
  });

  final String message;
  final VoidCallback onRetry;
  final TextTheme textTheme;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.error_outline, size: 40, color: AppColors.error),
          const SizedBox(height: 10),
          Text(message, style: AppTheme.bd(textTheme), textAlign: TextAlign.center),
          const SizedBox(height: 14),
          OutlinedButton(onPressed: onRetry, child: const Text('Try again')),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Shared / editor widgets
// ---------------------------------------------------------------------------

class _BackFab extends StatelessWidget {
  const _BackFab({required this.onPressed});
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface,
      shape: const CircleBorder(side: BorderSide(color: AppColors.hairline)),
      child: InkWell(
        onTap: onPressed,
        customBorder: const CircleBorder(),
        child: const SizedBox(
          width: 42,
          height: 42,
          child: Center(
            child: Icon(Icons.arrow_back_ios_new_rounded,
                size: 18, color: AppColors.onSurface),
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
        color: AppColors.surface,
        borderRadius: BorderRadius.circular(999),
        boxShadow: const [
          BoxShadow(
            color: Color(0x0F1C1D1A),
            blurRadius: 8,
            offset: Offset(0, 1),
          ),
        ],
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.open_with_rounded,
              size: 14, color: AppColors.primary),
          const SizedBox(width: 6),
          Text(
            'Move the map — the pin stays',
            style: AppTheme.cap(Theme.of(context).textTheme)
                .copyWith(fontWeight: FontWeight.w600),
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
      color: AppColors.surface,
      shape: const CircleBorder(side: BorderSide(color: AppColors.hairline)),
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
                : const Icon(Icons.my_location_rounded,
                    size: 20, color: AppColors.primary),
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
        color: AppColors.canvas,
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
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
                  color: AppColors.line,
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
                        .copyWith(fontWeight: FontWeight.w600),
                    maxLines: 1,
                    decoration: InputDecoration(
                      hintText: 'Street, building, landmark…',
                      isDense: true,
                      border: InputBorder.none,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
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
              Text(error!,
                  style:
                      AppTheme.sub(textTheme).copyWith(color: AppColors.error)),
            ],
            const SizedBox(height: 14),
            FilledButton(
              onPressed: saving ? null : onSave,
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
                disabledBackgroundColor: AppColors.fill,
                disabledForegroundColor: AppColors.onSurfaceMuted,
              ),
              child: Text(
                saving
                    ? 'Saving…'
                    : isEdit
                        ? 'Update location'
                        : 'Save location',
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w600,
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
            color: selected ? AppColors.primary : AppColors.hairline,
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
              color: selected ? AppColors.onPrimary : AppColors.onSurfaceMuted,
            ),
            const SizedBox(width: 6),
            Text(
              label,
              style: AppTheme.bd(Theme.of(context).textTheme).copyWith(
                fontWeight: FontWeight.w600,
                color: selected
                    ? AppColors.onPrimary
                    : AppColors.onSurfaceMuted,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Extension used by the list-mode background map
// ---------------------------------------------------------------------------

extension _LatLngX on CustomerLocation {
  LatLng toLatLng() => LatLng(lat, lng);
}

