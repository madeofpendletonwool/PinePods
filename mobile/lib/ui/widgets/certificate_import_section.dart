// Copyright 2025 the PinePods project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/services/security/certificate_manager.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';

/// Reusable UI for importing / removing custom TLS certificates:
///
///  * a **server CA** so a private / self-signed server certificate is trusted, and
///  * a **client certificate** (PKCS#12 `.p12`/`.pfx` + password) for mTLS.
///
/// Used both on the startup login screen (so certs can be trusted *before* the
/// first connection) and in Settings. State is driven by
/// [CertificateManager.instance.state].
class CertificateImportSection extends StatefulWidget {
  const CertificateImportSection({super.key});

  @override
  State<CertificateImportSection> createState() =>
      _CertificateImportSectionState();
}

class _CertificateImportSectionState extends State<CertificateImportSection> {
  final CertificateManager _cm = CertificateManager.instance;
  bool _busy = false;

  Future<void> _importServerCa() async {
    final file = await _pickFile(const ['pem', 'crt', 'cer', 'der']);
    if (file == null) return;
    final bytes = await _readBytes(file);
    if (bytes == null) {
      _snack(L.current.could_not_read_file);
      return;
    }
    final err = CertificateManager.validateServerCa(bytes);
    if (err != null) {
      _snack(err);
      return;
    }
    setState(() => _busy = true);
    await _cm.importServerCa(bytes, file.name);
    if (!mounted) return;
    setState(() => _busy = false);
    _snack(L.of(context)!.server_ca_imported(file.name));
  }

  Future<void> _importClientCert() async {
    final file = await _pickFile(const ['p12', 'pfx']);
    if (file == null) return;
    final bytes = await _readBytes(file);
    if (bytes == null) {
      _snack(L.current.could_not_read_file);
      return;
    }
    if (!mounted) return;
    final password = await _promptPassword();
    if (password == null) return; // cancelled
    final err = CertificateManager.validateClientCert(bytes, password);
    if (err != null) {
      _snack(err);
      return;
    }
    setState(() => _busy = true);
    await _cm.importClientCert(bytes, password, file.name);
    if (!mounted) return;
    setState(() => _busy = false);
    _snack(L.of(context)!.client_certificate_imported(file.name));
  }

  Future<PlatformFile?> _pickFile(List<String> extensions) async {
    try {
      return await FilePicker.pickFile(
        type: FileType.custom,
        allowedExtensions: extensions,
      );
    } catch (e) {
      _snack(L.of(context)!.could_not_open_file_picker(e.toString()));
      return null;
    }
  }

  Future<Uint8List?> _readBytes(PlatformFile file) async {
    try {
      return await file.readAsBytes();
    } catch (_) {
      return null;
    }
  }

  Future<String?> _promptPassword() {
    final controller = TextEditingController();
    return showDialog<String?>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: Text(L.of(context)!.certificate_password),
          content: TextField(
            controller: controller,
            obscureText: true,
            autofocus: true,
            decoration: InputDecoration(
              hintText: L.of(context)!.leave_blank_if_none,
            ),
            onSubmitted: (v) => Navigator.of(context).pop(v),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(null),
              child: Text(L.of(context)!.cancel_button_label),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(controller.text),
              child: Text(L.of(context)!.import),
            ),
          ],
        );
      },
    );
  }

  void _snack(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
  }

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<CertificateState>(
      valueListenable: _cm.state,
      builder: (context, state, _) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _certTile(
              icon: PhosphorIconsRegular.shieldCheck,
              title: L.of(context)!.server_ca_certificate,
              subtitle: state.serverCaName ??
                  L.of(context)!.server_ca_certificate_description,
              configured: state.hasServerCa,
              onImport: _importServerCa,
              onRemove: () async {
                await _cm.removeServerCa();
                _snack(L.current.server_ca_removed);
              },
            ),
            const SizedBox(height: 8),
            _certTile(
              icon: PhosphorIconsRegular.identificationBadge,
              title: L.of(context)!.client_certificate_mtls,
              subtitle: state.clientCertName ??
                  L.of(context)!.client_certificate_description,
              configured: state.hasClientCert,
              onImport: _importClientCert,
              onRemove: () async {
                await _cm.removeClientCert();
                _snack(L.current.client_certificate_removed);
              },
            ),
          ],
        );
      },
    );
  }

  Widget _certTile({
    required IconData icon,
    required String title,
    required String subtitle,
    required bool configured,
    required VoidCallback onImport,
    required VoidCallback onRemove,
  }) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Icon(icon, color: configured ? Colors.green : null),
      title: Text(title),
      subtitle: Text(subtitle, maxLines: 2, overflow: TextOverflow.ellipsis),
      trailing: _busy
          ? const SizedBox(
              height: 20, width: 20, child: CircularProgressIndicator(strokeWidth: 2))
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextButton(
                  onPressed: onImport,
                  child: Text(configured ? L.of(context)!.replace : L.of(context)!.import),
                ),
                if (configured)
                  IconButton(
                    icon: const Icon(PhosphorIconsRegular.trash),
                    tooltip: L.of(context)!.remove,
                    onPressed: onRemove,
                  ),
              ],
            ),
    );
  }
}
