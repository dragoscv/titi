package ro.titi.app.ui

import androidx.navigation3.runtime.NavKey
import kotlinx.serialization.Serializable

@Serializable sealed interface Route : NavKey
@Serializable data object Onboarding : Route
@Serializable data object Home : Route
@Serializable data class Group(val id: String) : Route
@Serializable data class Chat(val id: String) : Route
@Serializable data object Join : Route
@Serializable data object Settings : Route
