import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/auth_provider.dart';
import '../home/home_screen.dart';

class SeedPhraseScreen extends ConsumerStatefulWidget {
  final List<String> seedPhrase;
  final String username;
  final String password;

  const SeedPhraseScreen({
    super.key,
    required this.seedPhrase,
    required this.username,
    required this.password,
  });

  @override
  ConsumerState<SeedPhraseScreen> createState() => _SeedPhraseScreenState();
}

class _SeedPhraseScreenState extends ConsumerState<SeedPhraseScreen> {
  bool _hasCopied = false;
  bool _hasConfirmed = false;
  bool _isLoading = false;

  void _copyToClipboard() {
    final phrase = widget.seedPhrase.join(' ');
    Clipboard.setData(ClipboardData(text: phrase));

    setState(() => _hasCopied = true);

    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Frase copiada! Guarde em local seguro.'),
        behavior: SnackBarBehavior.floating,
      ),
    );
  }

  Future<void> _confirmAndContinue() async {
    if (!_hasConfirmed) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text('Confirme que salvou as palavras'),
          backgroundColor: Theme.of(context).colorScheme.error,
        ),
      );
      return;
    }

    setState(() => _isLoading = true);

    try {
      // Save credentials and complete registration with REAL seed
      await ref.read(authProvider.notifier).createAccount(
            username: widget.username,
            password: widget.password,
            seedWords: widget.seedPhrase,
          );

      if (!mounted) return;

      Navigator.of(context).pushAndRemoveUntil(
        MaterialPageRoute(builder: (context) => const HomeScreen()),
        (route) => false,
      );
    } catch (e) {
      if (!mounted) return;

      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro: $e'),
          backgroundColor: Theme.of(context).colorScheme.error,
        ),
      );
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Frase de Recuperação'),
      ),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Warning banner
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  color:
                      Theme.of(context).colorScheme.error.withOpacity(0.1),
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(
                    color:
                        Theme.of(context).colorScheme.error.withOpacity(0.3),
                  ),
                ),
                child: Row(
                  children: [
                    Icon(
                      Icons.warning_amber_rounded,
                      color: Theme.of(context).colorScheme.error,
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'MUITO IMPORTANTE',
                            style: Theme.of(context)
                                .textTheme
                                .titleSmall
                                ?.copyWith(
                                  color: Theme.of(context).colorScheme.error,
                                  fontWeight: FontWeight.bold,
                                ),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            'Estas palavras são a ÚNICA forma de recuperar sua conta. Se você perdê-las, perderá acesso a todos os seus arquivos.',
                            style:
                                Theme.of(context).textTheme.bodySmall?.copyWith(
                                      color:
                                          Theme.of(context).colorScheme.error,
                                    ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),

              // Instructions
              Text(
                'Anote estas ${widget.seedPhrase.length} palavras',
                style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.bold,
                    ),
              ),
              const SizedBox(height: 8),
              Text(
                'Escreva em papel e guarde em local seguro. Nunca compartilhe com ninguém.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: Theme.of(context)
                          .colorScheme
                          .onSurface
                          .withOpacity(0.7),
                    ),
              ),
              const SizedBox(height: 24),

              // Seed phrase grid
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  color: Theme.of(context).cardColor,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(
                    color: Theme.of(context).dividerColor,
                  ),
                ),
                child: Column(
                  children: [
                    GridView.builder(
                      shrinkWrap: true,
                      physics: const NeverScrollableScrollPhysics(),
                      gridDelegate:
                          const SliverGridDelegateWithFixedCrossAxisCount(
                        crossAxisCount: 3,
                        childAspectRatio: 2.5,
                        crossAxisSpacing: 8,
                        mainAxisSpacing: 8,
                      ),
                      itemCount: widget.seedPhrase.length,
                      itemBuilder: (context, index) {
                        return Container(
                          decoration: BoxDecoration(
                            color: Theme.of(context)
                                .colorScheme
                                .primary
                                .withOpacity(0.1),
                            borderRadius: BorderRadius.circular(8),
                          ),
                          child: Center(
                            child: RichText(
                              text: TextSpan(
                                children: [
                                  TextSpan(
                                    text: '${index + 1}. ',
                                    style: Theme.of(context)
                                        .textTheme
                                        .bodySmall
                                        ?.copyWith(
                                          color: Theme.of(context)
                                              .colorScheme
                                              .primary
                                              .withOpacity(0.6),
                                        ),
                                  ),
                                  TextSpan(
                                    text: widget.seedPhrase[index],
                                    style: Theme.of(context)
                                        .textTheme
                                        .bodyMedium
                                        ?.copyWith(
                                          fontWeight: FontWeight.w600,
                                          color: Theme.of(context)
                                              .colorScheme
                                              .primary,
                                        ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        );
                      },
                    ),
                    const SizedBox(height: 16),
                    OutlinedButton.icon(
                      onPressed: _copyToClipboard,
                      icon: Icon(
                        _hasCopied ? Icons.check : Icons.copy,
                      ),
                      label: Text(
                        _hasCopied ? 'Copiado!' : 'Copiar para área de transferência',
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),

              // Security tips
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  color: Theme.of(context)
                      .colorScheme
                      .tertiary
                      .withOpacity(0.1),
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Icon(
                          Icons.tips_and_updates_outlined,
                          color: Theme.of(context).colorScheme.tertiary,
                          size: 20,
                        ),
                        const SizedBox(width: 8),
                        Text(
                          'Dicas de segurança',
                          style:
                              Theme.of(context).textTheme.titleSmall?.copyWith(
                                    fontWeight: FontWeight.bold,
                                    color:
                                        Theme.of(context).colorScheme.tertiary,
                                  ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 12),
                    _SecurityTip(
                      icon: Icons.edit_note,
                      text: 'Escreva em papel, não salve digitalmente',
                    ),
                    _SecurityTip(
                      icon: Icons.lock,
                      text: 'Guarde em local seguro como cofre',
                    ),
                    _SecurityTip(
                      icon: Icons.group_off,
                      text: 'Nunca compartilhe com ninguém',
                    ),
                    _SecurityTip(
                      icon: Icons.photo_camera_outlined,
                      text: 'Não tire foto ou print',
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),

              // Confirmation checkbox
              CheckboxListTile(
                value: _hasConfirmed,
                onChanged: (value) {
                  setState(() => _hasConfirmed = value ?? false);
                },
                title: Text(
                  'Eu salvei minhas palavras de recuperação em local seguro',
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
                controlAffinity: ListTileControlAffinity.leading,
                contentPadding: EdgeInsets.zero,
              ),
              const SizedBox(height: 16),

              // Continue button
              ElevatedButton(
                onPressed: _isLoading ? null : _confirmAndContinue,
                child: _isLoading
                    ? const SizedBox(
                        width: 24,
                        height: 24,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Text('Continuar'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _SecurityTip extends StatelessWidget {
  final IconData icon;
  final String text;

  const _SecurityTip({
    required this.icon,
    required this.text,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Icon(
            icon,
            size: 16,
            color: Theme.of(context).colorScheme.tertiary.withOpacity(0.7),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              text,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color:
                        Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
                  ),
            ),
          ),
        ],
      ),
    );
  }
}
