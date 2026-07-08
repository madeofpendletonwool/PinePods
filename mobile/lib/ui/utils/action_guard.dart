/// Guards a set of async actions so only one runs at a time; a call made
/// while another is still in flight is dropped instead of firing a second,
/// overlapping request.
///
/// Extracted as a plain Dart class (rather than inlined bool + setState in a
/// widget's State) so it can be unit tested without any widget/Provider
/// scaffolding.
class ActionGuard {
  bool _inProgress = false;

  bool get inProgress => _inProgress;

  /// Runs [action] unless another guarded action is already in flight, in
  /// which case this is a no-op. [onChange] fires synchronously right after
  /// [inProgress] flips true, and again once it flips back to false (even if
  /// [action] throws) - wire it to `setState` to keep a button's
  /// enabled/disabled look in sync.
  Future<void> run(Future<void> Function() action, {required void Function() onChange}) async {
    if (_inProgress) return;
    _inProgress = true;
    onChange();
    try {
      await action();
    } finally {
      _inProgress = false;
      onChange();
    }
  }
}
