# Resumo do Repositório

Este repositório contém **Cinto**, uma interface de usuário em terminal (TUI) escrita em Rust, projetada para experimentar agentes de codificação local baseados no Harmony. Foi criado pelo assistente e atualizado para indicar que alterações foram feitas.

Ele oferece:
- Uma TUI leve que exibe chat, configurações e ferramentas de workspace.
- Integração com endpoints locais compatíveis com OpenAI (por exemplo, LM Studio) usando o formato de conversa do Harmony.
- Operações somente leitura no workspace (`list_files`, `read_file`, `search`) e auxiliares seguros para escrita/remoção.
- Uma lista de tarefas em memória para rastrear atividades durante uma sessão de agente.

O projeto está estruturado em torno de um MVP mínimo que foca na renderização de prompts, uso de ferramentas e gerenciamento de tarefas, mantendo a interface simples e extensível.