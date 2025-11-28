import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/auth_provider.dart';
import '../home/home_screen.dart';

class RecoverAccountScreen extends ConsumerStatefulWidget {
  const RecoverAccountScreen({super.key});

  @override
  ConsumerState<RecoverAccountScreen> createState() =>
      _RecoverAccountScreenState();
}

class _RecoverAccountScreenState extends ConsumerState<RecoverAccountScreen> {
  final _formKey = GlobalKey<FormState>();
  final List<TextEditingController> _wordControllers = List.generate(
    12,
    (index) => TextEditingController(),
  );
  final List<FocusNode> _focusNodes = List.generate(
    12,
    (index) => FocusNode(),
  );

  bool _isLoading = false;
  String? _errorMessage;

  @override
  void dispose() {
    for (final controller in _wordControllers) {
      controller.dispose();
    }
    for (final node in _focusNodes) {
      node.dispose();
    }
    super.dispose();
  }

  Future<void> _recoverAccount() async {
    if (!_formKey.currentState!.validate()) return;

    // Check all words are filled
    final words = _wordControllers.map((c) => c.text.trim().toLowerCase()).toList();
    if (words.any((w) => w.isEmpty)) {
      setState(() => _errorMessage = 'Preencha todas as palavras');
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final seedPhrase = words.join(' ');

      await ref.read(authProvider.notifier).recoverAccount(
            seedPhrase: seedPhrase,
          );

      if (!mounted) return;

      Navigator.of(context).pushAndRemoveUntil(
        MaterialPageRoute(builder: (context) => const HomeScreen()),
        (route) => false,
      );
    } catch (e) {
      if (!mounted) return;

      setState(() {
        _errorMessage = 'Frase de recuperação inválida. Verifique as palavras.';
      });
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  void _pasteFromClipboard() async {
    // TODO: Implement paste from clipboard and parse words
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Cole a frase e ela será distribuída automaticamente'),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Recuperar Conta'),
      ),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24.0),
          child: Form(
            key: _formKey,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // Header
                Text(
                  'Digite suas palavras de recuperação',
                  style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                ),
                const SizedBox(height: 8),
                Text(
                  'Insira as 12 palavras na ordem correta para recuperar sua conta.',
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                        color: Theme.of(context)
                            .colorScheme
                            .onSurface
                            .withOpacity(0.7),
                      ),
                ),
                const SizedBox(height: 24),

                // Paste button
                OutlinedButton.icon(
                  onPressed: _pasteFromClipboard,
                  icon: const Icon(Icons.paste),
                  label: const Text('Colar da área de transferência'),
                ),
                const SizedBox(height: 24),

                // Word inputs grid
                GridView.builder(
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: 3,
                    childAspectRatio: 2.0,
                    crossAxisSpacing: 8,
                    mainAxisSpacing: 8,
                  ),
                  itemCount: 12,
                  itemBuilder: (context, index) {
                    return _WordInput(
                      index: index,
                      controller: _wordControllers[index],
                      focusNode: _focusNodes[index],
                      onNext: () {
                        if (index < 11) {
                          _focusNodes[index + 1].requestFocus();
                        } else {
                          _focusNodes[index].unfocus();
                        }
                      },
                    );
                  },
                ),
                const SizedBox(height: 24),

                // Error message
                if (_errorMessage != null)
                  Container(
                    padding: const EdgeInsets.all(16),
                    decoration: BoxDecoration(
                      color: Theme.of(context).colorScheme.error.withOpacity(0.1),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Row(
                      children: [
                        Icon(
                          Icons.error_outline,
                          color: Theme.of(context).colorScheme.error,
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            _errorMessage!,
                            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                                  color: Theme.of(context).colorScheme.error,
                                ),
                          ),
                        ),
                      ],
                    ),
                  ),

                if (_errorMessage != null) const SizedBox(height: 24),

                // Info box
                Container(
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.primary.withOpacity(0.1),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Row(
                    children: [
                      Icon(
                        Icons.info_outline,
                        color: Theme.of(context).colorScheme.primary,
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          'A recuperação pode demorar alguns segundos enquanto sincronizamos com a rede.',
                          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                                color: Theme.of(context).colorScheme.primary,
                              ),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 32),

                // Recover button
                ElevatedButton(
                  onPressed: _isLoading ? null : _recoverAccount,
                  child: _isLoading
                      ? const SizedBox(
                          width: 24,
                          height: 24,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Text('Recuperar Conta'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _WordInput extends StatelessWidget {
  final int index;
  final TextEditingController controller;
  final FocusNode focusNode;
  final VoidCallback onNext;

  const _WordInput({
    required this.index,
    required this.controller,
    required this.focusNode,
    required this.onNext,
  });

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      controller: controller,
      focusNode: focusNode,
      textInputAction: index < 11 ? TextInputAction.next : TextInputAction.done,
      onFieldSubmitted: (_) => onNext(),
      autocorrect: false,
      enableSuggestions: false,
      style: Theme.of(context).textTheme.bodySmall,
      decoration: InputDecoration(
        contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
        prefixText: '${index + 1}. ',
        prefixStyle: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.primary.withOpacity(0.6),
            ),
        hintText: 'palavra',
        hintStyle: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurface.withOpacity(0.3),
            ),
      ),
      validator: (value) {
        if (value == null || value.trim().isEmpty) {
          return '';
        }
        return null;
      },
    );
  }
}
