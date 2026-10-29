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

  Future<bool> setEnabled(bool value) async {
    if (!isAndroid) return false;
    await init();
    if (textureId < 0) {
      enabled.value = false;
      return false;
    }
    try {
      final ok = await _channel.invokeMethod('setSurfaceDecode', value);
      enabled.value = ok == true && value;
      return enabled.value;
    } catch (e) {
      debugPrint('[Surface] setSurfaceDecode failed: $e');
      enabled.value = false;
      return false;
    }
  }

  void markFirstFrameShown() {
    gFFI.ffiModel.waitForFirstImage.value = false;
  }
}
