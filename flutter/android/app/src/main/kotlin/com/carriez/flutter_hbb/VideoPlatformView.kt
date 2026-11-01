package com.carriez.flutter_hbb

import android.content.Context
import android.graphics.SurfaceTexture
import android.view.Surface
import android.view.TextureView
import android.view.View
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.StandardMessageCodec
import io.flutter.plugin.platform.PlatformView
import io.flutter.plugin.platform.PlatformViewFactory
import ffi.FFI

/**
 * Hosts the decoded video in a TextureView so MediaCodec renders straight into
 * a GPU surface. Flutter only positions it, which keeps the picture, the canvas
 * transform and the gesture layer in sync.
 */
class VideoPlatformView(context: Context) : PlatformView {
    private val textureView = TextureView(context)
    private var surface: Surface? = null

    init {
        textureView.surfaceTextureListener = object : TextureView.SurfaceTextureListener {
            override fun onSurfaceTextureAvailable(
                surfaceTexture: SurfaceTexture,
                width: Int,
                height: Int
            ) {
                attach(surfaceTexture)
            }

            override fun onSurfaceTextureSizeChanged(
                surfaceTexture: SurfaceTexture,
                width: Int,
                height: Int
            ) {
            }

            override fun onSurfaceTextureDestroyed(surfaceTexture: SurfaceTexture): Boolean {
                release()
                return true
            }

            override fun onSurfaceTextureUpdated(surfaceTexture: SurfaceTexture) {}
        }
    }

    private fun attach(surfaceTexture: SurfaceTexture) {
        release()
        val newSurface = Surface(surfaceTexture)
        surface = newSurface
        FFI.setVideoSurface(newSurface)
    }

    private fun release() {
        FFI.setVideoSurface(null)
        surface?.release()
        surface = null
    }

    override fun getView(): View = textureView

    override fun dispose() {
        release()
    }
}

class VideoPlatformViewFactory(private val messenger: BinaryMessenger) :
    PlatformViewFactory(StandardMessageCodec.INSTANCE) {
    override fun create(context: Context, id: Int, args: Any?): PlatformView =
        VideoPlatformView(context)
}
