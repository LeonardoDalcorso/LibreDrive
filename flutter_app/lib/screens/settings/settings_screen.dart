import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/auth_provider.dart';
import '../../providers/storage_provider.dart';
import '../auth/welcome_screen.dart';

class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final auth = ref.watch(authProvider);
    final storage = ref.watch(storageQuotaProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Ajustes'),
      ),
      body: ListView(
        children: [
          // Profile section
          _ProfileCard(
            username: auth.username ?? 'Usuário',
            publicId: auth.publicId ?? '',
          ),

          const _SectionHeader(title: 'Armazenamento'),

          _SettingsTile(
            icon: Icons.storage,
            title: 'Espaço oferecido',
            subtitle: '${storage.offeredMb ~/ 1024} GB para a rede',
            onTap: () => _showStorageSettings(context, ref),
          ),

          _SettingsTile(
            icon: Icons.data_usage,
            title: 'Limite de uso',
            subtitle: '${storage.quotaMb ~/ 1024} GB disponíveis',
            onTap: () => _showStorageSettings(context, ref),
          ),

          const _SectionHeader(title: 'Rede P2P'),

          _SettingsTile(
            icon: Icons.wifi,
            title: 'Bootstrap Nodes',
            subtitle: 'Configurar nós de entrada',
            onTap: () {
              // TODO: Bootstrap settings
            },
          ),

          _SettingsTile(
            icon: Icons.router,
            title: 'Relay & NAT',
            subtitle: 'Ativado',
            onTap: () {
              // TODO: NAT settings
            },
          ),

          _SettingsTile(
            icon: Icons.favorite,
            title: 'Heartbeat',
            subtitle: 'A cada 24 horas',
            onTap: () {
              // TODO: Heartbeat settings
            },
          ),

          const _SectionHeader(title: 'Segurança'),

          _SettingsTile(
            icon: Icons.key,
            title: 'Frase de recuperação',
            subtitle: 'Ver suas 12 palavras',
            onTap: () => _showRecoveryPhrase(context),
          ),

          _SettingsTile(
            icon: Icons.lock,
            title: 'Alterar senha',
            subtitle: 'Atualizar senha local',
            onTap: () {
              // TODO: Change password
            },
          ),

          _SettingsTile(
            icon: Icons.fingerprint,
            title: 'Biometria',
            subtitle: 'Desativado',
            onTap: () {
              // TODO: Biometric settings
            },
          ),

          const _SectionHeader(title: 'Sobre'),

          _SettingsTile(
            icon: Icons.info_outline,
            title: 'Versão',
            subtitle: '1.0.0',
            onTap: null,
          ),

          _SettingsTile(
            icon: Icons.code,
            title: 'Código fonte',
            subtitle: 'github.com/libredrive',
            onTap: () {
              // TODO: Open GitHub
            },
          ),

          _SettingsTile(
            icon: Icons.article_outlined,
            title: 'Documentação',
            subtitle: 'Como usar o LibreDrive',
            onTap: () {
              // TODO: Open docs
            },
          ),

          const SizedBox(height: 16),

          // Logout button
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: OutlinedButton.icon(
              onPressed: () => _confirmLogout(context, ref),
              icon: const Icon(Icons.logout, color: Colors.red),
              label: const Text(
                'Sair da conta',
                style: TextStyle(color: Colors.red),
              ),
              style: OutlinedButton.styleFrom(
                side: const BorderSide(color: Colors.red),
              ),
            ),
          ),

          const SizedBox(height: 32),
        ],
      ),
    );
  }

  void _showStorageSettings(BuildContext context, WidgetRef ref) {
    final storage = ref.read(storageQuotaProvider);

    showModalBottomSheet(
      context: context,
      builder: (context) => _StorageSettingsSheet(
        currentOfferedGb: storage.offeredMb ~/ 1024,
        onSave: (gb) {
          ref.read(storageQuotaProvider.notifier).setOffered(gb * 1024);
          Navigator.pop(context);
        },
      ),
    );
  }

  void _showRecoveryPhrase(BuildContext context) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Atenção'),
        content: const Text(
          'Para ver sua frase de recuperação, você precisa confirmar sua senha.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancelar'),
          ),
          ElevatedButton(
            onPressed: () {
              Navigator.pop(context);
              // TODO: Show recovery phrase after password verification
            },
            child: const Text('Continuar'),
          ),
        ],
      ),
    );
  }

  void _confirmLogout(BuildContext context, WidgetRef ref) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Sair da conta?'),
        content: const Text(
          'Você precisará das suas 12 palavras de recuperação para acessar novamente. Tem certeza que quer sair?',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancelar'),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: Colors.red,
            ),
            onPressed: () async {
              Navigator.pop(context);
              await ref.read(authProvider.notifier).logout();
              if (context.mounted) {
                Navigator.of(context).pushAndRemoveUntil(
                  MaterialPageRoute(
                    builder: (context) => const WelcomeScreen(),
                  ),
                  (route) => false,
                );
              }
            },
            child: const Text('Sair'),
          ),
        ],
      ),
    );
  }
}

class _ProfileCard extends StatelessWidget {
  final String username;
  final String publicId;

  const _ProfileCard({
    required this.username,
    required this.publicId,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.all(16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            CircleAvatar(
              radius: 30,
              backgroundColor: Theme.of(context).colorScheme.primary,
              child: Text(
                username.isNotEmpty ? username[0].toUpperCase() : '?',
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 24,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    username,
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.bold,
                        ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    publicId.length > 20
                        ? '${publicId.substring(0, 20)}...'
                        : publicId,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: Theme.of(context)
                              .colorScheme
                              .onSurface
                              .withOpacity(0.6),
                        ),
                  ),
                ],
              ),
            ),
            IconButton(
              icon: const Icon(Icons.qr_code),
              onPressed: () {
                // TODO: Show QR code
              },
            ),
          ],
        ),
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String title;

  const _SectionHeader({required this.title});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleSmall?.copyWith(
              color: Theme.of(context).colorScheme.primary,
              fontWeight: FontWeight.bold,
            ),
      ),
    );
  }
}

class _SettingsTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback? onTap;

  const _SettingsTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(icon, color: Theme.of(context).colorScheme.primary),
      title: Text(title),
      subtitle: Text(subtitle),
      trailing: onTap != null
          ? const Icon(Icons.chevron_right)
          : null,
      onTap: onTap,
    );
  }
}

class _StorageSettingsSheet extends StatefulWidget {
  final int currentOfferedGb;
  final Function(int) onSave;

  const _StorageSettingsSheet({
    required this.currentOfferedGb,
    required this.onSave,
  });

  @override
  State<_StorageSettingsSheet> createState() => _StorageSettingsSheetState();
}

class _StorageSettingsSheetState extends State<_StorageSettingsSheet> {
  late double _sliderValue;

  @override
  void initState() {
    super.initState();
    _sliderValue = widget.currentOfferedGb.toDouble();
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              'Espaço oferecido à rede',
              style: Theme.of(context).textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.bold,
                  ),
            ),
            const SizedBox(height: 8),
            Text(
              'Quanto mais espaço você oferece, mais pode usar.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context)
                        .colorScheme
                        .onSurface
                        .withOpacity(0.7),
                  ),
            ),
            const SizedBox(height: 32),
            Center(
              child: Text(
                '${_sliderValue.toInt()} GB',
                style: Theme.of(context).textTheme.headlineLarge?.copyWith(
                      fontWeight: FontWeight.bold,
                      color: Theme.of(context).colorScheme.primary,
                    ),
              ),
            ),
            Slider(
              value: _sliderValue,
              min: 1,
              max: 100,
              divisions: 99,
              label: '${_sliderValue.toInt()} GB',
              onChanged: (value) {
                setState(() => _sliderValue = value);
              },
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('1 GB'),
                Text('100 GB'),
              ],
            ),
            const SizedBox(height: 24),
            ElevatedButton(
              onPressed: () => widget.onSave(_sliderValue.toInt()),
              child: const Text('Salvar'),
            ),
          ],
        ),
      ),
    );
  }
}
