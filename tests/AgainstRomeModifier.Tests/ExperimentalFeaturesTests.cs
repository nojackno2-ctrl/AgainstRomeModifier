namespace AgainstRomeModifier.Tests
{
    public class ExperimentalFeaturesTests : IDisposable
    {
        private readonly string _configDir;
        private readonly string _configFile;

        public ExperimentalFeaturesTests()
        {
            // Redirect persistence to an isolated temporary file. Tests must never
            // read or overwrite the user's real %APPDATA% settings.
            _configDir = Path.Combine(Path.GetTempPath(), "AgainstRomeModifier.Tests", Guid.NewGuid().ToString("N"));
            _configFile = Path.Combine(_configDir, "settings.json");
            Loc.OverrideSettingsFileForTesting(_configFile);
        }

        public void Dispose()
        {
            GC.SuppressFinalize(this);
            Loc.OverrideSettingsFileForTesting(null);
            try
            {
                if (Directory.Exists(_configDir))
                {
                    Directory.Delete(_configDir, recursive: true);
                }
            }
            catch
            {
                // ignore
            }
        }

        [Fact]
        public void Loc_IsPromoted_InitiallyFalse()
        {
            // Arrange
            string testFeatureId = "Test_Experimental_Feature_" + Guid.NewGuid();

            // Act & Assert
            Assert.False(Loc.IsPromoted(testFeatureId));
        }

        [Fact]
        public void Loc_PromoteAndDemote_StateChangesAndPersists()
        {
            // Arrange
            string testFeatureId = "Test_Experimental_Feature_" + Guid.NewGuid();

            // Act - Promote
            Loc.PromoteFeature(testFeatureId);

            // Assert
            Assert.True(Loc.IsPromoted(testFeatureId));
            Assert.Contains(testFeatureId, Loc.PromotedFeatures);

            // Assert file persistence
            Assert.True(File.Exists(_configFile));
            string json = File.ReadAllText(_configFile);
            Assert.Contains(testFeatureId, json);

            // Act - Demote
            Loc.DemoteFeature(testFeatureId);

            // Assert
            Assert.False(Loc.IsPromoted(testFeatureId));
            Assert.DoesNotContain(testFeatureId, Loc.PromotedFeatures);
            
            // Assert file persistence reflects change
            json = File.ReadAllText(_configFile);
            Assert.DoesNotContain(testFeatureId, json);
        }

        [Fact]
        public void AppSettings_SerializationRoundTrip()
        {
            // Arrange
            string testFeatureId1 = "FeatA";
            string testFeatureId2 = "FeatB";
            
            Loc.PromoteFeature(testFeatureId1);
            Loc.PromoteFeature(testFeatureId2);

            // Act - Re-load preference to simulate startup
            Loc.ReloadLanguage();

            // Assert
            Assert.True(Loc.IsPromoted(testFeatureId1));
            Assert.True(Loc.IsPromoted(testFeatureId2));

            // Clean up
            Loc.DemoteFeature(testFeatureId1);
            Loc.DemoteFeature(testFeatureId2);
        }
    }
}
