import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hive_flutter/hive_flutter.dart';

import 'screens/splash_screen.dart';
import 'services/rust_bridge.dart';
import 'services/database_service.dart';
import 'services/secure_storage_service.dart';
import 'theme/app_theme.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Initialize Hive for local storage
  await Hive.initFlutter();

  // Initialize database service
  await DatabaseService.initialize();

  // Initialize secure storage service (encryption)
  await SecureStorageService.initialize();

  // Initialize Rust bridge
  await RustBridge.initialize();

  runApp(
    const ProviderScope(
      child: LibreDriveApp(),
    ),
  );
}

class LibreDriveApp extends StatelessWidget {
  const LibreDriveApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'LibreDrive',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: ThemeMode.system,
      home: const SplashScreen(),
    );
  }
}
