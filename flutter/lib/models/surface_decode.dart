import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';

/// Drives the Android zero-copy decode path.
///
/// The picture itself lives in a native TextureView (see VideoPlatformView);
/// MediaCodec renders straight into its Surface, so no frame ever crosses to
/// the Dart side as RGBA. This class only watches for the first rendered frame
/// and tells the UI when it is safe to drop the software-rendered fallback.
class SurfaceDecode {
  SurfaceDecode._();

  static final SurfaceDecode instance = SurfaceDecode._();

  static const _channel = MethodChannel('org.rustdesk.rustdesk/host');

  final enabled = false.obs;
  final size = ''.obs;

  bool _watching = false;

  /// Polls the decoder until a frame reached the surface. Safe to call more
  /// than once; only one watch runs at a time.
  Future<bool> startWatch() async {
    if (!isAndroid || _watching) return enabled.value;
    _watching = true;
    try {
      for (var i = 0; i < 40; i++) {
        await Future.delayed(const Duration(milliseconds: 500));
        try {
          final ready = await _channel.invokeMethod('isSurfaceFrameReady');
          if (ready == true) {
            await _refreshSize();
            enabled.value = true;
            return true;
          }
        } catch (e) {
          debugPrint('[Surface] probe failed: $e');
          return false;
        }
      }
      debugPrint('[Surface] no frame in time, keeping software render');
      return false;
    } finally {
      _watching = false;
    }
  }

  void reset() {
    enabled.value = false;
    size.value = '';
  }

  Future<void> _refreshSize() async {
    try {
      final value = await _channel.invokeMethod('getVideoSurfaceSize');
      if (value is String && value.contains('x')) {
        size.value = value;
      }
    } catch (_) {}
  }

  void markFirstFrameShown() {
    gFFI.ffiModel.waitForFirstImage.value = false;
  }
}
