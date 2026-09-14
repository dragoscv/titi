# kotlinx.serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** { *** Companion; }
-keepclasseswithmembers class kotlinx.serialization.json.** { kotlinx.serialization.KSerializer serializer(...); }
-keep,includedescriptorclasses class ro.titi.app.**$$serializer { *; }
-keepclassmembers class ro.titi.app.** { *** Companion; }
-keepclasseswithmembers class ro.titi.app.** { kotlinx.serialization.KSerializer serializer(...); }
