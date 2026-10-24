use dioxus::prelude::*;

#[component]
pub fn AboutView(logo_uri: &'static str) -> Element {
    rsx! {
        div {
            // Hero Card
            div {
                class: "surface-pad",
                style: "text-align: center; padding: 22px 16px; margin-bottom: 14px;",
                img {
                    src: "{logo_uri}",
                    alt: "CrabCheck Logo",
                    style: "width: 58px; height: 58px; border-radius: 14px; object-fit: contain; margin-bottom: 10px; box-shadow: 0 4px 12px rgba(0,0,0,0.06);",
                }
                h2 {
                    style: "margin: 0 0 4px 0; font-size: 20px; font-weight: 700; color: #101828; letter-spacing: -0.02em;",
                    "CrabCheck"
                }
                div {
                    style: "display: inline-block; padding: 3px 9px; background: #FEF3F2; color: #B42318; border: 1px solid #FECDCA; border-radius: 999px; font-size: 11px; font-weight: 600; margin-bottom: 8px;",
                    "v0.1.0 • Release Oficial"
                }
                p {
                    style: "margin: 0; font-size: 13px; color: #475467; line-height: 1.45;",
                    "Despertador & Rastreador de Hábitos à Prova de Falhas"
                }

                div {
                    style: "display: flex; justify-content: center; flex-wrap: wrap; gap: 6px; margin-top: 14px;",
                    span {
                        style: "font-size: 11px; font-weight: 500; background: #F2F4F7; color: #344054; padding: 4px 8px; border-radius: 6px;",
                        "🦀 Rust + Android"
                    }
                    span {
                        style: "font-size: 11px; font-weight: 500; background: #ECFDF3; color: #027A48; padding: 4px 8px; border-radius: 6px;",
                        "🔒 100% Offline"
                    }
                    span {
                        style: "font-size: 11px; font-weight: 500; background: #EFF8FF; color: #175CD3; padding: 4px 8px; border-radius: 6px;",
                        "⚡ SQLite Local"
                    }
                }
            }

            // Propósito
            div {
                class: "surface-pad",
                style: "margin-bottom: 14px;",
                div {
                    style: "display: flex; align-items: center; gap: 8px; margin-bottom: 8px;",
                    span { style: "font-size: 16px;", "🎯" }
                    h3 {
                        style: "margin: 0; font-size: 14px; font-weight: 600; color: #101828;",
                        "Propósito & Disciplina"
                    }
                }
                p {
                    style: "margin: 0; font-size: 13px; color: #475467; line-height: 1.5;",
                    "Criado para blindar os seus compromissos diários de "
                    strong { "leitura e estudo" }
                    ". Em vez de alarmes que são desligados no piloto automático, o CrabCheck exige ação física real: você se levanta, apanha o livro e valida com a câmara."
                }
            }

            // Funcionalidades em Destaque
            div {
                class: "surface-group",
                style: "margin-bottom: 14px;",
                div {
                    style: "padding: 12px 14px; border-bottom: 1px solid #EAECF0; background: #F9FAFB;",
                    h3 {
                        style: "margin: 0; font-size: 13px; font-weight: 600; color: #344054; text-transform: uppercase; letter-spacing: 0.04em;",
                        "Recursos Implementados"
                    }
                }

                div {
                    class: "list-row",
                    div { style: "font-size: 18px;", "📖" }
                    div {
                        style: "flex: 1;",
                        div {
                            style: "font-size: 13px; font-weight: 600; color: #101828;",
                            "Lembrete Prévio de 15 Minutos"
                        }
                        div {
                            style: "font-size: 12px; color: #667085; margin-top: 2px;",
                            "Notificação com vibração 15 minutos antes do horário para preparar o livro e os materiais de estudo."
                        }
                    }
                }

                div {
                    class: "list-row",
                    div { style: "font-size: 18px;", "📸" }
                    div {
                        style: "flex: 1;",
                        div {
                            style: "font-size: 13px; font-weight: 600; color: #101828;",
                            "Visão Computacional no Aparelho"
                        }
                        div {
                            style: "font-size: 12px; color: #667085; margin-top: 2px;",
                            "Google ML Kit rodando localmente a 30 FPS. Reconhecimento afinado para livros, garrafas e objetos com botão de lanterna para baixa luz."
                        }
                    }
                }

                div {
                    class: "list-row",
                    div { style: "font-size: 18px;", "🔒" }
                    div {
                        style: "flex: 1;",
                        div {
                            style: "font-size: 13px; font-weight: 600; color: #101828;",
                            "Alarme Rigoroso (Nível 3)"
                        }
                        div {
                            style: "font-size: 12px; color: #667085; margin-top: 2px;",
                            "Ecrã de bloqueio e toque insistente que só param quando a prova visual for confirmada pela câmara."
                        }
                    }
                }

                div {
                    class: "list-row",
                    div { style: "font-size: 18px;", "⏳" }
                    div {
                        style: "flex: 1;",
                        div {
                            style: "font-size: 13px; font-weight: 600; color: #101828;",
                            "Proteção Anti-Sabotagem (24h)"
                        }
                        div {
                            style: "font-size: 12px; color: #667085; margin-top: 2px;",
                            "Bloqueio assimétrico que impede apagar ou editar hábitos em momentos de fraqueza, com opção de forçar quando necessário."
                        }
                    }
                }

                div {
                    class: "list-row",
                    div { style: "font-size: 18px;", "📝" }
                    div {
                        style: "flex: 1;",
                        div {
                            style: "font-size: 13px; font-weight: 600; color: #101828;",
                            "Linha do Tempo & Notas Rápidas"
                        }
                        div {
                            style: "font-size: 12px; color: #667085; margin-top: 2px;",
                            "Histórico diário com fotos das provas e anotações rápidas com filtros por hashtags (#leitura, #estudo)."
                        }
                    }
                }
            }

            // Privacidade & Soberania
            div {
                class: "surface-pad",
                style: "margin-bottom: 14px;",
                div {
                    style: "display: flex; align-items: center; gap: 8px; margin-bottom: 8px;",
                    span { style: "font-size: 16px;", "🛡️" }
                    h3 {
                        style: "margin: 0; font-size: 14px; font-weight: 600; color: #101828;",
                        "Privacidade & Armazenamento Local"
                    }
                }
                p {
                    style: "margin: 0 0 10px 0; font-size: 13px; color: #475467; line-height: 1.5;",
                    "O CrabCheck é "
                    strong { "100% offline" }
                    ". Não existem contas, logins, servidores ou telemetria. Suas fotos de prova e hábitos nunca saem do seu telemóvel."
                }
                div {
                    style: "background: #F9FAFB; border: 1px solid #EAECF0; border-radius: 8px; padding: 10px; font-family: monospace; font-size: 11px; color: #344054; word-break: break-all;",
                    "📁 Banco SQLite: crabcheck.db\n📍 /data/user/0/com.crabcheck.app/files/crabcheck.db"
                }
            }

            // Arquitetura Técnica
            div {
                class: "surface-pad",
                style: "margin-bottom: 20px;",
                div {
                    style: "display: flex; align-items: center; gap: 8px; margin-bottom: 8px;",
                    span { style: "font-size: 16px;", "⚙️" }
                    h3 {
                        style: "margin: 0; font-size: 14px; font-weight: 600; color: #101828;",
                        "Arquitetura de Engenharia"
                    }
                }
                ul {
                    style: "margin: 0; padding-left: 18px; font-size: 12px; color: #475467; line-height: 1.6;",
                    li { strong { "Rust (Dioxus + Rusqlite):" } " Lógica de domínio, integridade do banco SQLite e interface reativa." }
                    li { strong { "Kotlin (Android AAR):" } " AlarmManager exato, Foreground Service com prioridade máxima e CameraX." }
                    li { strong { "UniFFI:" } " Conexão binária ultrarrápida entre Rust e Android sem overhead de rede." }
                    li { strong { "Google ML Kit:" } " Modelo de visão computacional embutido e executado no processador local." }
                }
            }

            // Rodapé
            div {
                style: "text-align: center; padding: 8px 0 16px 0; font-size: 12px; color: #98A2B3;",
                p { style: "margin: 0 0 4px 0;", "CrabCheck • Construído para foco e consistência." }
                p { style: "margin: 0; font-size: 11px;", "Código aberto & local-first." }
            }
        }
    }
}
