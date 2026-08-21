#ifndef RUNNER_SYSTEM_MEDIA_H_
#define RUNNER_SYSTEM_MEDIA_H_

#include <windows.h>
#include <windows.media.h>
#include <wrl/client.h>

#include "media_status.h"

// Le cuenta a Windows que NeoTube es un reproductor de audio.
//
// Integración con los System Media Transport Controls (SMTC): centro de control,
// teclas multimedia, pantalla de bloqueo.
class SystemMediaControls {
 public:
  SystemMediaControls() = default;
  ~SystemMediaControls();

  SystemMediaControls(const SystemMediaControls&) = delete;
  SystemMediaControls& operator=(const SystemMediaControls&) = delete;

  bool Start(HWND window, UINT mensaje);
  void Stop();

  void Update(const EstadoMultimedia& estado);

 private:
  void ActualizarLinea(const EstadoMultimedia& estado);

  HWND window_ = nullptr;
  UINT mensaje_ = 0;

  Microsoft::WRL::ComPtr<ABI::Windows::Media::ISystemMediaTransportControls>
      smtc_;
  EventRegistrationToken token_boton_ = {};
  EventRegistrationToken token_salto_ = {};
};

#endif  // RUNNER_SYSTEM_MEDIA_H_
