use dioxus::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionHealthState {
    pub exact_alarm: bool,
    pub overlay: bool,
    pub accessibility: bool,
    pub camera: bool,
    pub notifications: bool,
    pub battery: bool,
}

impl PermissionHealthState {
    pub fn all_granted_default() -> Self {
        Self {
            exact_alarm: true,
            overlay: true,
            accessibility: true,
            camera: true,
            notifications: true,
            battery: true,
        }
    }

    pub fn from_csv(csv: &str) -> Self {
        let mut state = Self {
            exact_alarm: false,
            overlay: false,
            accessibility: false,
            camera: false,
            notifications: false,
            battery: false,
        };
        for token in csv.split(',') {
            match token.trim() {
                "exact_alarm=1" => state.exact_alarm = true,
                "overlay=1" => state.overlay = true,
                "accessibility=1" => state.accessibility = true,
                "camera=1" => state.camera = true,
                "notifications=1" => state.notifications = true,
                "battery=1" => state.battery = true,
                _ => {}
            }
        }
        state
    }

    pub fn all_granted(&self) -> bool {
        self.exact_alarm
            && self.overlay
            && self.accessibility
            && self.camera
            && self.notifications
            && self.battery
    }

    pub fn granted_count(&self) -> usize {
        let mut c = 0;
        if self.exact_alarm { c += 1; }
        if self.overlay { c += 1; }
        if self.accessibility { c += 1; }
        if self.camera { c += 1; }
        if self.notifications { c += 1; }
        if self.battery { c += 1; }
        c
    }

    pub fn missing_labels(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.exact_alarm { out.push("Alarme Exato"); }
        if !self.overlay { out.push("Sobreposição"); }
        if !self.accessibility { out.push("Acessibilidade"); }
        if !self.camera { out.push("Câmara"); }
        if !self.notifications { out.push("Notificações"); }
        if !self.battery { out.push("Otimização de Bateria"); }
        out
    }
}

#[component]
pub fn HealthView(
    current_health: PermissionHealthState,
    on_refresh: EventHandler<()>,
    on_open_setting: EventHandler<&'static str>,
) -> Element {
    let granted_count = current_health.granted_count();

    rsx! {
        div {
            div {
                class: "surface-pad",
                style: "display: flex; justify-content: space-between; align-items: center; gap: 12px;",
                div {
                    div {
                        style: "font-size: 15px; font-weight: 600; color: #101828;",
                        "Permissões do Android ({granted_count}/6)"
                    }
                    p {
                        style: "margin: 3px 0 0 0; font-size: 12px; color: #667085; line-height: 1.4;",
                        "Necessárias para disparo exato, sobreposição de ecrã e validação por câmara."
                    }
                }
                button {
                    class: "btn-secondary",
                    style: "cursor: pointer;",
                    onclick: move |_| on_refresh.call(()),
                    "Atualizar"
                }
            }

            div {
                class: "surface-group",
                for (key, name, desc, is_ok) in [
                    (
                        "exact_alarm",
                        "Alarme exato",
                        "Dispara no minuto agendado mesmo com o ecrã em repouso.",
                        current_health.exact_alarm,
                    ),
                    (
                        "overlay",
                        "Sobrepor a outras aplicações",
                        "Permite abrir o ecrã de alarme sobre qualquer aplicação.",
                        current_health.overlay,
                    ),
                    (
                        "accessibility",
                        "Serviço de acessibilidade",
                        "Mantém o ecrã de validação focado durante alarmes Nível 3.",
                        current_health.accessibility,
                    ),
                    (
                        "camera",
                        "Câmara",
                        "Utilizada localmente para reconhecer o objeto do hábito.",
                        current_health.camera,
                    ),
                    (
                        "notifications",
                        "Notificações de ecrã inteiro",
                        "Mantém o serviço de som ativo e acorda o dispositivo.",
                        current_health.notifications,
                    ),
                    (
                        "battery",
                        "Sem restrição de bateria",
                        "Evita que o sistema interrompa o agendamento em segundo plano.",
                        current_health.battery,
                    ),
                ] {
                    div {
                        key: "perm-{key}",
                        class: "list-row",
                        div {
                            div {
                                style: "display: flex; align-items: center; gap: 8px;",
                                span {
                                    class: if is_ok { "dot-ok" } else { "dot-warn" }
                                }
                                span {
                                    style: "font-size: 14px; font-weight: 500; color: #101828;",
                                    "{name}"
                                }
                            }
                            div {
                                style: "font-size: 12px; color: #667085; margin-top: 2px; padding-left: 14px;",
                                "{desc}"
                            }
                        }
                        button {
                            class: if is_ok { "btn-secondary" } else { "btn-primary" },
                            style: "padding: 6px 11px; font-size: 12px; flex-shrink: 0; cursor: pointer;",
                            onclick: move |_| on_open_setting.call(key),
                            if is_ok { "Rever" } else { "Ativar" }
                        }
                    }
                }
            }
        }
    }
}
