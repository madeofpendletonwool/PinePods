// Copyright 2025 the PinePods project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/services/security/certificate_manager.dart';

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
      _snack('Could not read the selected file.');
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
    _snack('Server CA imported: ${file.name}');
  }

  Future<void> _importClientCert() async {
    final file = await _pickFile(const ['p12', 'pfx']);
    if (file == null) return;
    final bytes = await _readBytes(file);
    if (bytes == null) {
      _snack('Could not read the selected file.');
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
    _snack('Client certificate imported: ${file.name}');
  }

  Future<PlatformFile?> _pickFile(List<String> extensions) async {
    try {
      return await FilePicker.pickFile(
        type: FileType.custom,
        allowedExtensions: extensions,
      );
    } catch (e) {
      _snack('Could not open file picker: $e');
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
          title: const Text('Certificate password'),
          content: TextField(
            controller: controller,
            obscureText: true,
            autofocus: true,
            decoration: const InputDecoration(
              hintText: 'Leave blank if none',
            ),
            onSubmitted: (v) => Navigator.of(context).pop(v),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(null),
              child: const Text('Cancel'),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(controller.text),
              child: const Text('Import'),
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
              icon: Icons.verified_user_outlined,
              title: 'Server CA certificate',
              subtitle: state.serverCaName ??
                  'For private or self-signed server certificates',
              configured: state.hasServerCa,
              onImport: _importServerCa,
              onRemove: () async {
                await _cm.removeServerCa();
                _snack('Server CA removed (restart to fully apply).');
              },
            ),
            const SizedBox(height: 8),
            _certTile(
              icon: Icons.badge_outlined,
              title: 'Client certificate (mTLS)',
              subtitle: state.clientCertName ??
                  'A .p12 / .pfx certificate to present to the server',
              configured: state.hasClientCert,
              onImport: _importClientCert,
              onRemove: () async {
                await _cm.removeClientCert();
                _snack('Client certificate removed (restart to fully apply).');
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
                  child: Text(configured ? 'Replace' : 'Import'),
                ),
                if (configured)
                  IconButton(
                    icon: const Icon(Icons.delete_outline),
                    tooltip: 'Remove',
                    onPressed: onRemove,
                  ),
              ],
            ),
    );
  }
}
