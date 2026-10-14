using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativeObjectDefinitionsTests
{
    [Theory]
    [InlineData("400", "1", "900", 100)]
    [InlineData("0x400", "-1", "900", 100)]
    [InlineData("F000441", "2147483647", "900", 100)]
    [InlineData("4", "1", "900", 0)]
    [InlineData("400", "0", "900", 0)]
    [InlineData("400", "1", "-1", 0)]
    [InlineData("400", "1", "2500", 0)]
    [InlineData("400", "1", "899", 0)]
    public void Creation_field_uses_typus_index_active_and_low_flags_bit10(string flags, string active, string index, int expected)
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_Typus_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
        try
        {
            // Configured literal prefix: this must not load CUSTOM/typus.dau.
            File.WriteAllText(Path.Combine(root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nCUSTOM\n");
            string[] fields = Enumerable.Repeat("0", 61).ToArray();
            fields[0] = "555"; fields[1] = "1"; fields[54] = index;
            File.WriteAllText(Path.Combine(root, "CUSTOMobjdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
            File.WriteAllText(Path.Combine(root, "CUSTOMtypus.d00"), "[TypusDefaults]\n900,1,name,0\n");
            File.WriteAllText(Path.Combine(root, "CUSTOMtypus.d02"), $"[TypusDefaults]\n,,,900,,{active},,name,{flags}\n");
            File.WriteAllText(Path.Combine(root, "CUSTOMtypus.d09"), $"[TypusDefaults]\n900,{active}\n[Other]\n900,broken\n");
            File.WriteAllText(Path.Combine(root, "CUSTOMtypus.d10"), "[TypusDefaults]\n900,0\n");
            var before = Directory.GetFiles(root, "*", SearchOption.AllDirectories).ToDictionary(p => p, File.ReadAllBytes);
            var definition = NativeObjectDefinitions.Load(root)[555];
            Assert.Equal(short.Parse(index), definition.Typus);
            Assert.Equal((ushort)expected, definition.InitialField1c);
            foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
            File.Delete(Path.Combine(root, "CUSTOMtypus.d00"));
            File.Delete(Path.Combine(root, "CUSTOMtypus.d02"));
            File.Delete(Path.Combine(root, "CUSTOMtypus.d09"));
            Assert.Equal((ushort)0, NativeObjectDefinitions.Load(root)[555].InitialField1c);
        }
        finally { Directory.Delete(root, true); }
    }

    [Theory]
    [InlineData("900,broken,name,400")]
    [InlineData("900,2147483648,name,400")]
    [InlineData("900,1,name,100000000")]
    [InlineData("900,1,name,xyz")]
    [InlineData("2500,1,name,400")]
    public void Invalid_present_typus_is_rejected(string row)
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_Typus_" + Guid.NewGuid().ToString("N"));
        string directory = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(directory);
        try
        {
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), Row(555, 1, 0));
            File.WriteAllText(Path.Combine(directory, "typus.dau"), "[TypusDefaults]\n" + row);
            Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Load(root));
        }
        finally { Directory.Delete(root, true); }
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Configured_directory_and_numbered_overlays_preserve_omitted_fields(bool configured)
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        try
        {
            string directory = Path.Combine(root, configured ? "CUSTOM" : "SYSTEM/DATA_MP/DEFAULTS");
            Directory.CreateDirectory(directory);
            if (configured)
            {
                Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
                File.WriteAllText(Path.Combine(root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nCUSTOM\\ ; source\n");
                string wrong = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS"); Directory.CreateDirectory(wrong);
                File.WriteAllText(Path.Combine(wrong, "objdef.dau"), Row(555, 1, -1));
            }
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), Row(555, 1, 0));
            File.WriteAllText(Path.Combine(directory, "objdef.d02"), Row(555, 1, -1));
            File.WriteAllText(Path.Combine(directory, "objdef.d09"), "[ObjectDefaults]\n555,0\n[Unrelated]\n999,broken\n");
            File.WriteAllText(Path.Combine(directory, "objdef.d10"), Row(555, 1, 0));
            var before = Directory.GetFiles(root, "*", SearchOption.AllDirectories).ToDictionary(p => p, File.ReadAllBytes);
            var definition = NativeObjectDefinitions.Load(root)[555];
            Assert.False(definition.Active); Assert.Equal(-1, definition.Afstr);
            foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
            File.WriteAllText(Path.Combine(directory, "objdef.d09"), Row(555, 1, 1));
            Assert.True(NativeObjectDefinitions.Load(root)[555].RequiresThirdPosition);
        }
        finally { if (Directory.Exists(root)) Directory.Delete(root, true); }
    }

    [Theory]
    [InlineData("missing")]
    [InlineData("broken")]
    public void Optional_overlay_can_define_new_type_and_invalid_present_layer_is_not_ignored(string fault)
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        string directory = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(directory);
        try
        {
            File.WriteAllText(Path.Combine(directory, "objdef.d00"), Row(770, 1, 1));
            Assert.True(NativeObjectDefinitions.Load(root)[770].RequiresThirdPosition);
            if (fault == "broken")
            {
                File.WriteAllText(Path.Combine(directory, "objdef.d01"), "[ObjectDefaults]\n770,broken\n");
                Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Load(root));
            }
        }
        finally { Directory.Delete(root, true); }
    }

    [Fact]
    public void DefaultPath_is_a_literal_prefix_and_does_not_insert_a_separator()
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
        try
        {
            File.WriteAllText(Path.Combine(root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nCUSTOM\n");
            File.WriteAllText(Path.Combine(root, "CUSTOMobjdef.dau"), Row(770, 1, 1));
            string directory = Path.Combine(root, "CUSTOM"); Directory.CreateDirectory(directory);
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), Row(770, 1, 0));
            Assert.True(NativeObjectDefinitions.Load(root)[770].RequiresThirdPosition);
        }
        finally { Directory.Delete(root, true); }
    }

    [Theory]
    [InlineData("leading")]
    [InlineData("middle")]
    [InlineData("trailing")]
    public void Native_comma_runs_do_not_consume_logical_columns(string gap)
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        string directory = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(directory);
        try
        {
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), Row(770, 0, 0));
            string row = Row(770, 1, -1);
            row = gap switch
            {
                "leading" => row.Replace("770,", ",,,770,"),
                "middle" => row.Replace("770,1,", "770,,,1,,"),
                _ => row.TrimEnd() + ",,,\n"
            };
            File.WriteAllText(Path.Combine(directory, "objdef.d09"), row);
            var definition = NativeObjectDefinitions.Load(root)[770];
            Assert.True(definition.Active);
            Assert.Equal(-1, definition.Afstr);
        }
        finally { Directory.Delete(root, true); }
    }

    [Fact]
    public void Whitespace_token_is_not_an_absent_active_field()
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        string directory = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(directory);
        try
        {
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), Row(770, 1, 1));
            File.WriteAllText(Path.Combine(directory, "objdef.d09"), "[ObjectDefaults]\n770, ,0\n");
            Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Load(root));
        }
        finally { Directory.Delete(root, true); }
    }

    [Fact]
    public void Vitals_use_signed_integer_columns_and_partial_overlays_keep_previous_values()
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        string directory = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(directory);
        try
        {
            string[] fields = Enumerable.Repeat("0", 61).ToArray();
            fields[0] = "770"; fields[1] = "1"; fields[19] = "2147483647"; fields[21] = "-1";
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
            File.WriteAllText(Path.Combine(directory, "objdef.d09"), "[ObjectDefaults]\n770,1\n771,1\n");
            var definitions = NativeObjectDefinitions.Load(root);
            Assert.Equal(int.MaxValue, definitions[770].InitialHp);
            Assert.Equal(-1, definitions[770].InitialMorale);
            Assert.Equal(0, definitions[771].InitialHp);
            Assert.Equal(0, definitions[771].InitialMorale);
        }
        finally { Directory.Delete(root, true); }
    }

    [Theory]
    [InlineData("2147483648")]
    [InlineData("broken")]
    public void Invalid_vitals_cannot_be_ignored(string value)
    {
        string[] fields = Enumerable.Repeat("0", 61).ToArray();
        fields[0] = "770"; fields[1] = "1"; fields[19] = value;
        Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Parse("[ObjectDefaults]\n" + string.Join(',', fields)));
    }

    [Theory]
    [InlineData("en-US")]
    [InlineData("de-DE")]
    [InlineData("fr-FR")]
    public void Moves_and_maxruhm_overlay_fields_keep_omitted_values_across_cultures(string culture)
    {
        string root = Path.Combine(Path.GetTempPath(), "ARM_ObjDef_" + Guid.NewGuid().ToString("N"));
        string directory = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(directory);
        var previousCulture = System.Globalization.CultureInfo.CurrentCulture;
        try
        {
            System.Globalization.CultureInfo.CurrentCulture = new(culture);
            string[] fields = Enumerable.Repeat("0", 153).ToArray(); fields[0] = "770"; fields[1] = "1";
            fields[4] = "1.75"; fields[152] = "-2147483648";
            File.WriteAllText(Path.Combine(directory, "objdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
            File.WriteAllText(Path.Combine(directory, "objdef.d09"), "[ObjectDefaults]\n770,1,0,0,2.5e-1\n771,1\n");
            var definitions = NativeObjectDefinitions.Load(root);
            Assert.Equal(.25f, definitions[770].InitialMoves);
            Assert.Equal(int.MinValue, definitions[770].InitialMaxRuhm);
            Assert.Equal(0f, definitions[771].InitialMoves);
            Assert.Equal(0, definitions[771].InitialMaxRuhm);
        }
        finally
        {
            System.Globalization.CultureInfo.CurrentCulture = previousCulture;
            Directory.Delete(root, true);
        }
    }

    [Theory]
    [InlineData(4, "NaN")]
    [InlineData(4, "Infinity")]
    [InlineData(4, "1e100")]
    [InlineData(4, "broken")]
    [InlineData(152, "2147483648")]
    [InlineData(152, "broken")]
    public void Invalid_new_creation_scalars_cannot_be_ignored(int column, string value)
    {
        string[] fields = Enumerable.Repeat("0", 153).ToArray(); fields[0] = "770"; fields[1] = "1"; fields[column] = value;
        Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Parse("[ObjectDefaults]\n" + string.Join(',', fields)));
    }

    private static string Row(int id, int active, int afstr)
    {
        string[] fields = Enumerable.Repeat("0", 61).ToArray(); fields[0] = id.ToString(); fields[1] = active.ToString(); fields[60] = afstr.ToString();
        return "[ObjectDefaults]\n" + string.Join(',', fields) + "\n";
    }

    [Theory]
    [InlineData(0, true, false)]
    [InlineData(1, true, true)]
    [InlineData(-1, true, true)]
    [InlineData(1, false, false)]
    public void Afstr_is_signed_word_at_column_60_not_particle_geometry(int afstr, bool active, bool third)
    {
        string[] row = Enumerable.Repeat("0", 65).ToArray();
        row[0] = "770"; row[1] = active ? "1" : "0"; row[60] = afstr.ToString(); row[64] = "-1";
        var definition = NativeObjectDefinitions.Parse("[ObjectDefaults]\n; comment\n" + string.Join(',', row))[770];
        Assert.Equal(afstr, definition.Afstr); Assert.Equal(third, definition.RequiresThirdPosition);
    }

    [Theory]
    [InlineData("0", "32768")]
    [InlineData("2500", "1")]
    [InlineData("-1", "1")]
    [InlineData("0", "broken")]
    public void Invalid_type_fields_are_not_silently_treated_as_zero(string id, string afstr)
    {
        string[] row = Enumerable.Repeat("0", 61).ToArray(); row[0] = id; row[1] = "1"; row[60] = afstr;
        Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Parse("[ObjectDefaults]\n" + string.Join(',', row)));
    }
}
