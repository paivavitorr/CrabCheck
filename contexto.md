# Projeto: Agenda de Hábitos com Atrito Configurável

## A Ideia Central
A aplicação é uma fusão entre um diário de anotações em formato de linha do tempo (timeline) e um cronograma de alta rigidez. O grande diferencial é o sistema de **Atrito Configurável (Níveis de Alerta)** e **Prova de Ação**. Em vez de apenas notificar o utilizador sobre um evento, a aplicação exige provas no mundo real (como tirar a foto de um objeto específico) para desligar o alarme e confirmar que o hábito foi executado. O sistema também conta com um aspeto de cobrança social (accountability), onde falhas são reportadas a um grupo ou parceiro.

## Como Poderia Ser Usado (Casos de Uso)
O foco principal é combater a procrastinação e garantir a execução de rotinas diárias:
*   **Rotina Matinal (Obrigatória):** O utilizador agenda "Ginásio às 06:00" com nível máximo de alerta. Às 06:00, o telemóvel dispara um alarme intrusivo que bloqueia o ecrã. A única forma de o silenciar é abrindo a câmara da aplicação e tirando uma foto de um ténis desportivo ou de um haltere.
*   **Rotina de Trabalho (Informativa):** O utilizador agenda "Início do Expediente às 08:00" com nível de alerta baixo. A aplicação apenas envia uma notificação push comum, que pode ser descartada, servindo apenas como um balizador do dia.
*   **Construção de Hábito (Leitura):** O utilizador agenda "Ler um livro às 22:00" com nível de alerta máximo. O alarme só para de perturbar quando a câmara reconhece a capa de um livro.
*   **Contrato Social:** Dois amigos configuram a mesma meta de estudos diária. Se um deles não enviar a foto do caderno até à hora limite, o outro recebe uma notificação de que o parceiro falhou no compromisso.

## Lista de Funcionalidades

### Funcionalidades Principais (Core)
*   **Timeline Diária (Estilo Memos):** Interface principal onde o dia é visualizado de forma cronológica, misturando tarefas agendadas, pensamentos rápidos e registos de conclusão.
*   **Sistema de Alarmes em 3 Níveis:**
    *   *Nível 1 (Passivo):* Notificação padrão do sistema operativo.
    *   *Nível 2 (Persistente):* Alarme sonoro contínuo que exige interação prolongada no ecrã (ex: segurar um botão por 10 segundos) para ser desativado.
    *   *Nível 3 (Prova de Ação):* Bloqueio de ecrã com alarme. Exige a captura de uma imagem validada por Inteligência Artificial (ex: foto de um livro, ténis, garrafa de água) para ser desativado.
*   **Reconhecimento de Imagem Offline:** Validação da "Prova de Ação" usando a câmara em tempo real para identificar objetos.
*   **Sincronização Multiplataforma:** Acesso e notificação sincronizados entre Computador (Desktop) e Telemóvel (APK).

### Funcionalidades Potenciais (Expansão)
*   **Verificação por GPS:** Além da foto, exigir que o utilizador esteja num raio de 50 metros de um local específico (ex: morada do ginásio ou faculdade) para validar o check-in.
*   **Grupos de Accountability:** Criação de "clãs" ou duplas de rotina, com um feed partilhado mostrando quem cumpriu e quem falhou nas metas diárias.
*   **Dashboard de Estatísticas:** Gráficos semanais e mensais mostrando a taxa de sucesso por nível de alarme (ex: "Ignora 80% dos alertas de Nível 1, mas cumpre 95% dos alertas de Nível 3").
*   **Suporte a Markdown:** Permissão para escrever notas detalhadas e criar checklists dentro de cada bloco de horário da agenda.

## Stack Tecnológica e Arquitetura

### Frontend / Interface Multiplataforma
*   **Linguagem:** Rust
*   **Framework:** Dioxus
*   **Como será usado:** O Dioxus servirá como o núcleo visual e lógico da aplicação. Como uma única base de código em Rust pode ser direcionada para múltiplas plataformas, o Dioxus será utilizado para gerar os executáveis nativos para Desktop, a versão Web e as aplicações mobile (Android APK e iOS) através de comandos de build específicos (como `dx serve --platform desktop` e `dx serve --platform android`)[cite: 1]. Toda a interface de configuração de horários e contratos sociais será construída com componentes Dioxus.

### Backend e Base de Dados
*   **Plataforma:** Supabase (PostgreSQL) ou Firebase.
*   **Como será usado:** Responsável por armazenar os perfis, a agenda do utilizador e gerir a sincronização em tempo real. Essencial para a funcionalidade de "Contrato Social", garantindo que a falha ou o sucesso de um utilizador atualize instantaneamente a timeline do seu grupo de accountability. 

### Integrações Nativas e Inteligência Artificial
*   **Bibliotecas de FFI (Foreign Function Interface):** `jni` (Java Native Interface) para Android.
*   **Como será usado:** Como a aplicação exige o controlo de alarmes intrusivos e permissões de sobreposição de ecrã no telemóvel, o código Rust precisará de comunicar com as APIs nativas do Android via JNI para agendar "Background Services" e "AlarmManagers" que consigam acordar o dispositivo na hora exata.
*   **Motor de IA (Machine Learning):** Google ML Kit (Object Detection / Image Labeling).
*   **Como será usado:** Integrado na câmara do dispositivo para realizar a verificação da "Prova de Ação" do Nível 3. Quando a câmara for aberta, as frames serão passadas para o ML Kit (a correr localmente no aparelho para não depender de internet), que devolverá as labels do que está a ser "visto" (ex: "Book", "Shoe", "Dumbbell") para o código Rust libertar o bloqueio do alarme.
