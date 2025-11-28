# LibreDrive

<p align="center">
  <img src="docs/assets/logo.png" alt="LibreDrive Logo" width="200"/>
</p>

<p align="center">
  <strong>Armazenamento Descentralizado, Seguro e Privado</strong>
</p>

<p align="center">
  <a href="#sobre">Sobre</a> •
  <a href="#características">Características</a> •
  <a href="#arquitetura">Arquitetura</a> •
  <a href="#segurança">Segurança</a> •
  <a href="#instalação">Instalação</a> •
  <a href="#uso">Uso</a> •
  <a href="#roadmap">Roadmap</a>
</p>

---

## Sobre

**LibreDrive** é uma plataforma de armazenamento em nuvem descentralizada e peer-to-peer (P2P) que oferece privacidade total e resistência à censura. Diferente de serviços tradicionais como Google Drive ou Dropbox, seus arquivos são criptografados no seu dispositivo e distribuídos em fragmentos pela rede, garantindo que ninguém - nem mesmo os operadores da rede - possam acessar seus dados.

### Por que LibreDrive?

| Problema | Solução LibreDrive |
|----------|-------------------|
| Serviços centralizados podem acessar seus arquivos | Criptografia end-to-end antes do upload |
| Censura e remoção de conteúdo | Rede descentralizada sem controle central |
| Dependência de um único provedor | Dados distribuídos em múltiplos peers |
| Custos mensais crescentes | Modelo colaborativo: contribua espaço, ganhe armazenamento |
| Ponto único de falha | Redundância com erasure coding |

---

## Características

### Criptografia de Nível Militar
- **AES-256-GCM** - Mesmo padrão usado por governos e instituições financeiras
- **Chaves derivadas por arquivo** - Cada arquivo tem sua própria chave de criptografia
- **Zero-Knowledge** - Seus dados são criptografados antes de sair do dispositivo

### Arquitetura Blockchain-Style
- **SHA-256 Hashing** - Mesmo algoritmo usado no Bitcoin para verificação de integridade
- **Merkle Trees** - Estrutura de verificação criptográfica para detectar alterações
- **Double SHA-256** - Proteção extra para identificadores críticos

### Redundância Inteligente
- **Erasure Coding (10+4)** - Arquivo dividido em 14 fragmentos, recuperável com apenas 10
- **Tolerância a falhas** - Pode perder até 28% dos fragmentos e ainda recuperar 100% do arquivo
- **Distribuição geográfica** - Fragmentos espalhados pela rede P2P

### Rede P2P Descentralizada
- **libp2p** - Mesma tecnologia usada pelo IPFS e Filecoin
- **Kademlia DHT** - Tabela de hash distribuída para localização de arquivos
- **mDNS Discovery** - Descoberta automática de peers na rede local
- **Relay Support** - Conexão através de NAT e firewalls

---

## Arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│                        LibreDrive App                           │
│                     (Flutter + Dart)                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   UI Layer  │  │  Providers  │  │    Crypto Service       │ │
│  │  (Screens)  │◄─┤ (Riverpod)  │◄─┤  - AES-256-GCM          │ │
│  └─────────────┘  └─────────────┘  │  - SHA-256 Hashing      │ │
│                                     │  - Merkle Trees         │ │
│                                     │  - Key Derivation       │ │
│                                     └─────────────────────────┘ │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                   Secure Storage Service                    ││
│  │  - Encryption/Decryption  - Shard Management                ││
│  │  - Integrity Verification - File Metadata                   ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                        Rust Core                                │
│                    (libredrive_core)                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │   Identity   │  │    Crypto    │  │      Storage         │  │
│  │  - Keypairs  │  │  - AES-GCM   │  │  - File Manager      │  │
│  │  - BIP39     │  │  - Hashing   │  │  - Erasure Coding    │  │
│  │  - Signing   │  │  - HKDF      │  │  - Reed-Solomon      │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                      P2P Network                            ││
│  │  - libp2p Node      - Kademlia DHT    - Storage Protocol   ││
│  │  - Peer Discovery   - Message Relay   - Heartbeat System   ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │        Rede P2P Global        │
              │   (Outros peers LibreDrive)   │
              └───────────────────────────────┘
```

### Fluxo de Upload

```
1. Arquivo Original (ex: foto.jpg - 5MB)
              │
              ▼
2. Gerar Hash SHA-256 (identificador único)
   Hash: "a1b2c3d4e5f6..."
              │
              ▼
3. Dividir em Chunks (64KB cada)
   [Chunk 0] [Chunk 1] [Chunk 2] ... [Chunk 79]
              │
              ▼
4. Calcular Hash de cada Chunk
   [Hash 0] [Hash 1] [Hash 2] ... [Hash 79]
              │
              ▼
5. Construir Merkle Tree
              [Merkle Root]
             /              \
        [Hash 0-39]      [Hash 40-79]
           / \              / \
         ...              ...
              │
              ▼
6. Derivar Chave Única (HKDF)
   MasterKey + FileID → FileKey
              │
              ▼
7. Criptografar cada Chunk (AES-256-GCM)
   [Encrypted 0] [Encrypted 1] ... [Encrypted 79]
              │
              ▼
8. Erasure Coding (10+4 shards)
   [Shard 0] ... [Shard 9] [Parity 0] ... [Parity 3]
              │
              ▼
9. Distribuir Shards pela Rede P2P
   Peer A: [Shard 0, 3, 7]
   Peer B: [Shard 1, 4, 8]
   Peer C: [Shard 2, 5, 9]
   ...
```

### Fluxo de Download

```
1. Usuário solicita arquivo pelo ID
              │
              ▼
2. Buscar metadados na DHT
   - Lista de shards
   - Merkle Root
   - Peers que possuem shards
              │
              ▼
3. Solicitar shards aos peers
   (precisa de pelo menos 10 de 14)
              │
              ▼
4. Reconstruir arquivo (Reed-Solomon)
              │
              ▼
5. Verificar integridade (Merkle Tree)
              │
              ▼
6. Descriptografar (AES-256-GCM)
              │
              ▼
7. Verificar hash final
              │
              ▼
8. Arquivo Original restaurado
```

---

## Segurança

### Modelo de Ameaças

| Ameaça | Proteção |
|--------|----------|
| Interceptação de dados em trânsito | TLS + criptografia end-to-end |
| Acesso não autorizado aos arquivos | AES-256-GCM com chaves derivadas |
| Modificação maliciosa de dados | Merkle Tree + verificação de hash |
| Perda de dados por falha de peers | Erasure coding com redundância 40% |
| Análise de metadados | IDs de arquivo são hashes, não nomes |
| Ataque de força bruta | Chaves de 256 bits (2^256 combinações) |

### Especificações Criptográficas

```
Algoritmo de Criptografia: AES-256-GCM
  - Tamanho da Chave: 256 bits
  - Tamanho do Nonce: 96 bits (12 bytes)
  - Tag de Autenticação: 128 bits (16 bytes)

Algoritmo de Hash: SHA-256
  - Saída: 256 bits (32 bytes)
  - Uso: Identificação de arquivos, verificação de integridade

Derivação de Chaves: HKDF-SHA256
  - Input: Master Key (256 bits) + File ID
  - Output: File Key (256 bits)
  - Info: "libredrive-file-key-v1"

Erasure Coding: Reed-Solomon
  - Data Shards: 10
  - Parity Shards: 4
  - Overhead: 40%
  - Tolerância: Até 4 shards perdidos

Identidade: Ed25519
  - Chave Pública: 256 bits
  - Chave Privada: 512 bits
  - Seed Phrase: BIP39 (12 palavras)
```

### Comparação com Outras Soluções

| Feature | LibreDrive | Google Drive | Dropbox | IPFS | Filecoin |
|---------|-----------|--------------|---------|------|----------|
| Criptografia E2E | ✅ AES-256 | ❌ Server-side | ❌ Server-side | ❌ Opcional | ❌ Opcional |
| Descentralizado | ✅ P2P | ❌ | ❌ | ✅ | ✅ |
| Zero-Knowledge | ✅ | ❌ | ❌ | ❌ | ❌ |
| Redundância | ✅ Erasure | ✅ Replicação | ✅ Replicação | ✅ Replicação | ✅ Replicação |
| Resistente a Censura | ✅ | ❌ | ❌ | ✅ | ✅ |
| Chave por Arquivo | ✅ | ❌ | ❌ | ❌ | ❌ |
| Gratuito | ✅ | Limitado | Limitado | ✅ | ❌ Pago |
| Merkle Tree | ✅ | ❌ | ❌ | ✅ | ✅ |

---

## Tecnologias

### Frontend (Flutter App)
- **Flutter 3.24+** - Framework cross-platform
- **Dart** - Linguagem de programação
- **Riverpod** - Gerenciamento de estado
- **Hive** - Banco de dados local NoSQL
- **cryptography** - Biblioteca de criptografia Dart
- **pointycastle** - Implementações criptográficas

### Backend (Rust Core)
- **Rust** - Linguagem de sistemas de alta performance
- **libp2p** - Framework de rede P2P
- **aes-gcm** - Criptografia autenticada
- **reed-solomon-erasure** - Erasure coding
- **bip39** - Geração de seed phrases
- **tokio** - Runtime assíncrono

### Protocolos
- **Kademlia DHT** - Localização distribuída de dados
- **QUIC/TCP** - Transporte de rede
- **Noise Protocol** - Criptografia de canal
- **mDNS** - Descoberta em rede local

---

## Instalação

### Requisitos
- Flutter SDK 3.24+
- Rust 1.70+ (opcional, para P2P nativo)
- Chrome (para desenvolvimento web)

### Desenvolvimento

```bash
# Clonar repositório
git clone https://github.com/LeonardoDalcorso/LibreDrive.git
cd LibreDrive

# Instalar dependências Flutter
cd flutter_app
flutter pub get

# Rodar app (web)
flutter run -d chrome

# Rodar app (mobile)
flutter run

# Compilar core Rust (opcional, para P2P nativo)
cd ../rust_core
cargo build --release

# Rodar testes Rust
cargo test
```

### Build para Produção

```bash
# Web
flutter build web --release

# Android
flutter build apk --release

# iOS
flutter build ios --release

# Desktop
flutter build macos --release
flutter build windows --release
flutter build linux --release
```

---

## Estrutura do Projeto

```
LibreDrive/
├── flutter_app/                 # Aplicativo Flutter
│   ├── lib/
│   │   ├── main.dart           # Entry point
│   │   ├── screens/            # Telas do app
│   │   │   ├── home/           # Tela principal
│   │   │   ├── files/          # Gerenciador de arquivos
│   │   │   └── settings/       # Configurações
│   │   ├── providers/          # Estado (Riverpod)
│   │   │   ├── files_provider.dart
│   │   │   └── storage_provider.dart
│   │   ├── services/           # Serviços
│   │   │   ├── crypto_service.dart      # Criptografia
│   │   │   ├── secure_storage_service.dart
│   │   │   └── database_service.dart
│   │   └── theme/              # Tema visual
│   └── pubspec.yaml
│
├── rust_core/                   # Core em Rust
│   ├── src/
│   │   ├── crypto/             # Criptografia
│   │   │   ├── encryption.rs   # AES-256-GCM
│   │   │   └── hashing.rs      # SHA-256, Merkle
│   │   ├── identity/           # Identidade
│   │   │   ├── mod.rs          # UserIdentity
│   │   │   ├── seed.rs         # BIP39
│   │   │   └── keys.rs         # Ed25519
│   │   ├── storage/            # Armazenamento
│   │   │   ├── erasure.rs      # Reed-Solomon
│   │   │   └── file_manager.rs
│   │   └── p2p/                # Rede P2P
│   │       ├── node.rs         # libp2p node
│   │       ├── protocol.rs     # Protocolos
│   │       └── discovery.rs    # Peer discovery
│   └── Cargo.toml
│
├── docs/                        # Documentação
│   └── assets/                 # Imagens, logos
│
└── README.md                    # Este arquivo
```

---

## Uso

### Upload de Arquivo

1. Abra o app LibreDrive
2. Clique no botão **"+"**
3. Selecione **"Fazer Upload"**
4. Escolha o(s) arquivo(s)
5. Aguarde a criptografia e distribuição

O arquivo será automaticamente:
- Criptografado com AES-256-GCM
- Dividido em fragmentos (chunks)
- Protegido com Merkle Tree
- Fragmentado com erasure coding
- Armazenado com redundância

### Download de Arquivo

1. Navegue até o arquivo desejado
2. Clique no arquivo
3. Selecione **"Baixar"**

O sistema irá automaticamente:
- Localizar fragmentos na rede/local
- Reconstruir o arquivo (Reed-Solomon)
- Verificar integridade via Merkle Tree
- Descriptografar localmente

### Criar Pasta

1. Clique no botão **"+"**
2. Selecione **"Nova Pasta"**
3. Digite o nome
4. Clique em **"Criar"**

### Verificar Integridade

Todos os arquivos passam por verificação automática de integridade usando Merkle Trees. Se algum fragmento estiver corrompido, o sistema detecta e reconstrói a partir dos shards de paridade.

---

## Roadmap

### Fase 1 - MVP (Atual) ✅
- [x] Interface Flutter moderna
- [x] Criptografia AES-256-GCM
- [x] Hashing SHA-256 (blockchain-style)
- [x] Merkle Trees para integridade
- [x] Erasure coding para redundância
- [x] Armazenamento local seguro
- [x] Upload/Download funcional
- [x] Tema dark/light

### Fase 2 - P2P Network
- [ ] Integração completa Flutter-Rust (FFI)
- [ ] Conexão com peers via libp2p
- [ ] Distribuição real de shards
- [ ] Descoberta automática de peers (mDNS)
- [ ] Sistema de heartbeat (manter dados vivos)
- [ ] NAT traversal e relay

### Fase 3 - Features Avançadas
- [ ] Compartilhamento de arquivos (links criptografados)
- [ ] Sincronização entre dispositivos
- [ ] Versionamento de arquivos
- [ ] Busca por conteúdo (encrypted search)
- [ ] Aplicativo mobile nativo (iOS/Android)
- [ ] Cliente CLI

### Fase 4 - Economia
- [ ] Sistema de incentivos (tokens)
- [ ] Marketplace de armazenamento
- [ ] Contratos inteligentes para storage
- [ ] Pagamentos em criptomoeda
- [ ] DAO para governança

---

## Contribuindo

Contribuições são bem-vindas! Por favor, leia nosso guia de contribuição antes de submeter PRs.

```bash
# Fork o repositório
# Crie uma branch para sua feature
git checkout -b feature/nova-feature

# Faça commit das mudanças
git commit -m "Adiciona nova feature"

# Push para a branch
git push origin feature/nova-feature

# Abra um Pull Request
```

### Áreas que precisam de ajuda
- Testes automatizados
- Documentação
- Tradução (i18n)
- Design UI/UX
- Segurança/Auditoria
- Performance

---

## FAQ

### Meus arquivos estão seguros?
Sim. Todos os arquivos são criptografados com AES-256-GCM antes de sair do seu dispositivo. Nem mesmo os peers que armazenam fragmentos podem ler seu conteúdo.

### O que acontece se eu perder meu dispositivo?
Sua chave mestra está protegida no dispositivo. Em futuras versões, você poderá usar uma seed phrase (12 palavras) para recuperar sua conta em outro dispositivo.

### E se um peer sair da rede?
Graças ao erasure coding, seus arquivos podem ser reconstruídos mesmo com a perda de até 4 fragmentos (de 14). O sistema automaticamente redistribui fragmentos quando detecta peers offline.

### LibreDrive é gratuito?
Sim, o software é open source e gratuito. O modelo funciona na base de troca: você oferece espaço na rede e ganha direito a armazenar a mesma quantidade.

### Funciona offline?
Arquivos já baixados ficam em cache local e podem ser acessados offline. Novos uploads requerem conexão com a rede.

---

## Licença

Este projeto é licenciado sob a **MIT License** - veja o arquivo [LICENSE](LICENSE) para detalhes.

---

## Aviso Legal

Este é um projeto experimental em desenvolvimento ativo. Use por sua conta e risco. Sempre mantenha backup dos seus arquivos importantes.

**NUNCA compartilhe sua chave mestra ou seed phrase com ninguém.**

---

## Contato

- **GitHub**: [github.com/LeonardoDalcorso/LibreDrive](https://github.com/LeonardoDalcorso/LibreDrive)
- **Issues**: Para bugs e sugestões

---

<p align="center">
  <strong>LibreDrive</strong> - Seus Dados, Sua Privacidade, Sua Liberdade
</p>

<p align="center">
  Feito com ❤️ para um mundo mais descentralizado
</p>
