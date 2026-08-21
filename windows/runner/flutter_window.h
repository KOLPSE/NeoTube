#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include <memory>

#include "media_status.h"
#include "system_media.h"
#include "thumb_bar.h"
#include "win32_window.h"

// A window that does nothing but host a Flutter view.
class FlutterWindow : public Win32Window {
 public:
  // Creates a new FlutterWindow hosting a Flutter view running |project|.
  explicit FlutterWindow(const flutter::DartProject& project);
  virtual ~FlutterWindow();

 protected:
  // Win32Window:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  // Registra las teclas multimedia (play/pausa, siguiente, anterior) a nivel de sistema.
  void RegisterMediaKeys();
  void UnregisterMediaKeys();

  // Controles multimedia del sistema (panel del centro de control / SMTC).
  void StartSystemMedia();

  // Traduce lo que llega por el canal neotube/system_media.
  void OnSystemMediaCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  // Le cuenta a Dart que alguien ha pulsado algo en el panel multimedia.
  void EnviarComando(ComandoMultimedia comando, int64_t posicion_ms);

  // The project to run.
  flutter::DartProject project_;

  // The Flutter instance hosted by this window.
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;

  // Canal para teclas multimedia globales.
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      media_keys_channel_;
  bool media_keys_registered_ = false;

  // Canal para controles multimedia del sistema (SMTC).
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      system_media_channel_;
  SystemMediaControls system_media_;

  // Miniatura de la barra de tareas al pasar el ratón por encima: anterior,
  // play/pausa, siguiente. Comparte EstadoMultimedia/ComandoMultimedia con SMTC.
  ThumbBar thumb_bar_;
};

#endif  // RUNNER_FLUTTER_WINDOW_H_
