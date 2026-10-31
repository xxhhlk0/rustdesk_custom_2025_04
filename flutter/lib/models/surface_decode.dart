import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';

/// Drives the Android zero-copy decode path: MediaCodec renders straight into
/// the SurfaceTexture created by MainActivity, so no frame ever crosses to the
/// Dart side as RGBA.
class SurfaceDecode {
  SurfaceDecode._();

  static final SurfaceDecode instance = SurfaceDecode._();

  static const _channel = MethodChannel('org.rustdesk.rustdesk/host');

  int textureId = -1;
  final enabled = false.obs;

  bool get available => isAndroid && textureId >= 0;

  Future<void> init() async {
    if (!isAndroid || textureId >= 0) return;
    try {
      final id = await _channel.invokeMethod('getVideoTextureId');
      textureId = (id is num) ? id.toInt() : -1;
    } catch (e) {
      debugPrint('[Surface] getVideoTextureId failed: $e');
    }
  }

  Future<bool> setEnabled(bool value, {int width = 0, int height = 0}) async {
    if (!isAndroid) return false;
    if (!value) {
      enabled.value = false;
      try {
        await _channel.invokeMethod('setSurfaceDecode', {'enable': false});
      } catch (_) {}
      return false;
    }
    await init();
    if (textureId < 0) {
      enabled.value = false;
      return false;
    }
    try {
      final ok = await _channel.invokeMethod('setSurfaceDecode', {
        'enable': true,
        'width': width,
        'height': height,
      });
      if (ok != true) {
        enabled.value = false;
        return false;
      }
      // Only switch the UI over once the codec really produced a frame,
      // otherwise the texture would stay black.
      if (!await _waitForFrame()) {
        debugPrint('[Surface] no frame in time, falling back');
        await _channel.invokeMethod('setSurfaceDecode', {'enable': false});
        enabled.value = false;
        return false;
      }
      enabled.value = true;
      return true;
    } catch (e) {
      debugPrint('[Surface] setSurfaceDecode failed: $e');
      enabled.value = false;
      return false;
    }
  }

  Future<bool> _waitForFrame() async {
    // SPS/PPS only ride on key frames, so give the stream time to reach the
    // next IDR before giving up on the surface path.
    for (var i = 0; i < 30; i++) {
      await Future.delayed(const Duration(milliseconds: 500));
      try {
        final ready = await _channel.invokeMethod('isSurfaceFrameReady');
        if (ready == true) return true;
      } catch (_) {
        return false;
      }
    }
    return false;
  }

  void markFirstFrameShown() {
    gFFI.ffiModel.waitForFirstImage.value = false;
  }
}
