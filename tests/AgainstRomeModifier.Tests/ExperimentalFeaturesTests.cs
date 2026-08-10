namespace AgainstRomeModifier.Tests
{
    public class ExperimentalFeaturesTests : IDisposable
    {
        private readonly string _configDir;
        private readonly string _configFile;
        private readonly string _originalSettingsContent = null!;

        public ExperimentalFeaturesTests()
        {
            // 備份原有的設定檔，確保測試不會破壞本機真正的修改器設定
            string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            _configDir = Path.Combine(appData, "AgainstRomeModifier");
            _configFile = Path.Combine(_configDir, "settings.json");

            if (File.Exists(_configFile))
            {
                _originalSettingsContent = File.ReadAllText(_configFile);
            }
        }

        public void Dispose()
        {
            GC.SuppressFinalize(this);
            // 還原原有的設定檔
            try
            {
                if (_originalSettingsContent != null)
                {
                    if (!Directory.Exists(_configDir))
                    {
                        Directory.CreateDirectory(_configDir);
                    }
                    File.WriteAllText(_configFile, _originalSettingsContent);
                }
                else if (File.Exists(_configFile))
                {
                    File.Delete(_configFile);
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
